#![deny(missing_docs)]
//! A Conductor is a dynamically changing group of [Cell]s.
//!
//! A Conductor can be managed:
//! - externally, via an [`AppInterfaceApi`](super::api::AppInterfaceApi)
//! - from within a [`Cell`](super::Cell), via [`CellConductorApi`](super::api::CellConductorApi)
//!
//! In normal use cases, a single Holochain user runs a single Conductor in a single process.
//! However, there's no reason we can't have multiple Conductors in a single process, simulating multiple
//! users in a testing environment.
//!
//! ```rust, no_run
//! async fn async_main () {
//! use holochain_state::test_utils::test_db_dir;
//! use holochain::conductor::{Conductor, ConductorBuilder};
//! let env_dir = test_db_dir();
//! let conductor: Conductor = ConductorBuilder::new()
//!    .test(env_dir.path(), &[])
//!    .await
//!    .unwrap();
//!
//! // conductors are cloneable
//! let conductor2 = conductor.clone();
//!
//! assert_eq!(conductor.list_dnas(), vec![]);
//! conductor.shutdown();
//!
//! # }
//! ```
//!

pub use self::share::RwShare;
use super::api::RealAppInterfaceApi;
use super::api::ZomeCall;
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
use super::entry_def_store::get_entry_defs;
use super::error::ConductorError;
use super::interface::error::InterfaceResult;
use super::interface::websocket::spawn_admin_interface_task;
use super::interface::websocket::spawn_app_interface_task;
use super::interface::websocket::spawn_websocket_listener;
use super::interface::websocket::SIGNAL_BUFFER_SIZE;
use super::interface::AppInterfaceRuntime;
use super::interface::SignalBroadcaster;
use super::manager::keep_alive_task;
use super::manager::spawn_task_manager;
use super::manager::ManagedTaskAdd;
use super::manager::ManagedTaskHandle;
use super::manager::TaskManagerRunHandle;
use super::p2p_agent_store;
use super::p2p_agent_store::P2pBatch;
use super::p2p_agent_store::*;
use super::paths::DatabaseRootPath;
use super::ribosome_store::RibosomeStore;
use super::space::Space;
use super::space::Spaces;
use super::state::AppInterfaceConfig;
use super::state::AppInterfaceId;
use super::state::ConductorState;
use super::CellError;
use super::{api::RealAdminInterfaceApi, manager::TaskManagerClient};
use crate::conductor::cell::Cell;
use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorResult;
use crate::conductor::p2p_agent_store::get_single_agent_info;
use crate::conductor::p2p_agent_store::list_all_agent_info;
use crate::conductor::p2p_agent_store::query_peer_density;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::core::queue_consumer::QueueConsumerMap;
use crate::core::ribosome::guest_callback::post_commit::PostCommitArgs;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CHANNEL_BOUND;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CONCURRENT_LIMIT;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::ZomeCallResult;
use crate::{
    conductor::api::error::ConductorApiResult, core::ribosome::real_ribosome::RealRibosome,
};
pub use builder::*;
use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::FullIntegrationStateDump;
use holochain_conductor_api::FullStateDump;
use holochain_conductor_api::InstalledAppInfo;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::JsonDump;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use holochain_keystore::test_keystore::spawn_test_keystore;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::DnaHashExt;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::sql::sql_cell::state_dump;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::nonce::witness_nonce;
use holochain_state::nonce::WitnessNonceResult;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateMutationResult;
use holochain_state::prelude::StateQueryResult;
use holochain_state::prelude::*;
use holochain_state::source_chain;
use holochain_types::prelude::{test_keystore, wasm, *};
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p_types::config::JOIN_NETWORK_TIMEOUT;
use rusqlite::Transaction;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tracing::*;

#[cfg(any(test, feature = "test_utils"))]
use crate::core::queue_consumer::QueueTriggers;

pub use holochain_types::share;

mod builder;
pub use builder::*;

mod chc;
pub use chc::*;

pub use accessor_impls::*;
pub use app_impls::*;
pub use app_status_impls::*;
pub use cell_impls::*;
pub use clone_cell_impls::*;
pub use dna_impls::*;
pub use interface_impls::*;
pub use misc_impls::*;
pub use network_impls::*;
pub use scheduler_impls::*;
pub use startup_shutdown_impls::*;
pub use state_impls::*;

mod graft_records_onto_source_chain;

/// A list of Cells which failed to start, and why
pub type CellStartupErrors = Vec<(CellId, CellError)>;

/// Cloneable reference to a Conductor
pub type ConductorHandle = Arc<Conductor>;

/// The status of an installed Cell, which captures different phases of its lifecycle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellStatus {
    /// Kitsune knows about this Cell and it is considered fully "online"
    Joined,
    /// The Cell is on its way to being fully joined. It is a valid Cell from
    /// the perspective of the conductor, and can handle HolochainP2pEvents,
    /// but it is considered not to be fully running from the perspective of
    /// app status, i.e. if any app has a required Cell with this status,
    /// the app is considered to be in the Paused state.
    PendingJoin,

    /// The Cell is currently in the process of trying to join the network.
    Joining,
}

/// Declarative filter for CellStatus
pub type CellStatusFilter = CellStatus;

/// A [`Cell`] tracked by a Conductor, along with its [`CellStatus`]
struct CellItem {
    cell: Arc<Cell>,
    status: CellStatus,
}

impl CellItem {
    pub fn is_running(&self) -> bool {
        self.status == CellStatus::Joined
    }

    pub fn is_pending(&self) -> bool {
        self.status == CellStatus::PendingJoin
    }
}

pub(crate) type StopBroadcaster = tokio::sync::broadcast::Sender<()>;
pub(crate) type StopReceiver = tokio::sync::broadcast::Receiver<()>;

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    /// The collection of cells associated with this Conductor
    cells: RwShare<HashMap<CellId, CellItem>>,

    /// The config used to create this Conductor
    pub config: ConductorConfig,

    /// The map of dna hash spaces.
    pub(crate) spaces: Spaces,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: Arc<AtomicBool>,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: RwShare<Vec<u16>>,

    /// Collection app interface data, keyed by id
    app_interfaces: RwShare<HashMap<AppInterfaceId, AppInterfaceRuntime>>,

    /// The channels and handles needed to interact with the task_manager task.
    /// If this is None, then the task manager has not yet been initialized.
    pub(crate) task_manager: RwShare<Option<TaskManagerClient>>,

    /// Placeholder for what will be the real DNA/Wasm cache
    ribosome_store: RwShare<RibosomeStore>,

    /// Access to private keys for signing and encryption.
    keystore: MetaLairClient,

    /// Handle to the network actor.
    holochain_p2p: holochain_p2p::HolochainP2pRef,

    post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,

    scheduler: Arc<parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Conductor {
    /// Create a conductor builder
    pub fn builder() -> ConductorBuilder {
        ConductorBuilder::new()
    }
}

/// Methods related to conductor startup/shutdown
mod startup_shutdown_impls {
    use super::*;

    //-----------------------------------------------------------------------------
    /// Methods used by the [ConductorHandle]
    //-----------------------------------------------------------------------------
    impl Conductor {
        pub(crate) async fn witness_nonce_from_calling_agent(
            &self,
            agent: AgentPubKey,
            nonce: Nonce256Bits,
            expires: Timestamp,
        ) -> ConductorResult<WitnessNonceResult> {
            Ok(witness_nonce(
                &self.spaces.conductor_db,
                agent,
                nonce,
                Timestamp::now(),
                expires,
            )
            .await?)
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn new(
            config: ConductorConfig,
            ribosome_store: RwShare<RibosomeStore>,
            keystore: MetaLairClient,
            holochain_p2p: holochain_p2p::HolochainP2pRef,
            spaces: Spaces,
            post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,
        ) -> Self {
            Self {
                spaces,
                cells: RwShare::new(HashMap::new()),
                config,
                shutting_down: Arc::new(AtomicBool::new(false)),
                app_interfaces: RwShare::new(HashMap::new()),
                task_manager: RwShare::new(None),
                admin_websocket_ports: RwShare::new(Vec::new()),
                scheduler: Arc::new(parking_lot::Mutex::new(None)),
                ribosome_store,
                keystore,
                holochain_p2p,
                post_commit,
            }
        }

        /// A gate to put at the top of public functions to ensure that work is not
        /// attempted after a shutdown has been issued
        pub fn check_running(&self) -> ConductorResult<()> {
            if self
                .shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                Err(ConductorError::ShuttingDown)
            } else {
                Ok(())
            }
        }

        /// Broadcasts the shutdown signal to all managed tasks.
        /// To actually wait for these tasks to complete, be sure to
        /// `take_shutdown_handle` to await for completion.
        pub fn shutdown(&self) {
            self.shutting_down
                .store(true, std::sync::atomic::Ordering::Relaxed);

            use ghost_actor::GhostControlSender;
            let fut = self.holochain_p2p.ghost_actor_shutdown_immediate();
            tokio::task::spawn(fut);

            self.task_manager.share_ref(|tm| {
                if let Some(manager) = tm {
                    tracing::info!(
                        "Sending shutdown signal to {} managed tasks.",
                        manager.task_stop_broadcaster().receiver_count(),
                    );
                    manager
                        .task_stop_broadcaster()
                        .send(())
                        .map(|_| ())
                        .unwrap_or_else(|e| {
                            error!(?e, "Couldn't broadcast stop signal to managed tasks!");
                        })
                }
            });
        }

        /// Return the handle which waits for the task manager task to complete
        pub fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
            self.task_manager
                .share_mut(|tm| tm.as_mut().and_then(|manager| manager.take_handle()))
        }

        pub(crate) async fn initialize_conductor(
            self: Arc<Self>,
            admin_configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<CellStartupErrors> {
            self.load_dnas().await?;

            // Start the task manager
            let (task_add_sender, run_handle) = spawn_task_manager(self.clone());
            let (task_stop_broadcaster, _) = tokio::sync::broadcast::channel::<()>(1);
            self.task_manager.share_mut(|tm| {
                if tm.is_some() {
                    panic!("Cannot start task manager twice");
                }
                *tm = Some(TaskManagerClient::new(
                    task_add_sender,
                    task_stop_broadcaster,
                    run_handle,
                ));
            });

            self.clone().add_admin_interfaces(admin_configs).await?;
            self.clone().startup_app_interfaces().await?;

            // We don't care what fx are returned here, since all cells need to
            // be spun up
            let _ = self.start_paused_apps().await?;

            self.process_app_status_fx(AppStatusFx::SpinUp, None).await
        }
    }
}

/// Methods related to conductor interfaces
mod interface_impls {
    use super::*;

    impl Conductor {
        /// Spawn all admin interface tasks, register them with the TaskManager,
        /// and modify the conductor accordingly, based on the config passed in
        pub(crate) async fn add_admin_interfaces(
            self: Arc<Self>,
            configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<()> {
            let admin_api = RealAdminInterfaceApi::new(self.clone());
            let stop_tx = self.task_manager.share_ref(|tm| {
                tm.as_ref()
                    .expect("Task manager not started yet")
                    .task_stop_broadcaster()
                    .clone()
            });

            // Closure to process each admin config item
            let spawn_from_config = |AdminInterfaceConfig { driver, .. }| {
                let admin_api = admin_api.clone();
                let stop_tx = stop_tx.clone();
                async move {
                    match driver {
                        InterfaceDriver::Websocket { port } => {
                            let (listener_handle, listener) =
                                spawn_websocket_listener(port).await?;
                            let port = listener_handle.local_addr().port().unwrap_or(port);
                            let handle: ManagedTaskHandle = spawn_admin_interface_task(
                                listener_handle,
                                listener,
                                admin_api.clone(),
                                stop_tx.subscribe(),
                            )?;
                            InterfaceResult::Ok((port, handle))
                        }
                    }
                }
            };

            // spawn interface tasks, collect their JoinHandles,
            // panic on errors.
            let handles: Result<Vec<_>, _> =
                future::join_all(configs.into_iter().map(spawn_from_config))
                    .await
                    .into_iter()
                    .collect();
            // Exit if the admin interfaces fail to be created
            let handles = handles.map_err(Box::new)?;

            {
                let mut ports = Vec::new();

                // First, register the keepalive task, to ensure the conductor doesn't shut down
                // in the absence of other "real" tasks
                self.manage_task(ManagedTaskAdd::ignore(
                    tokio::spawn(keep_alive_task(stop_tx.subscribe())),
                    "keepalive task",
                ))
                .await?;

                // Now that tasks are spawned, register them with the TaskManager
                for (port, handle) in handles {
                    ports.push(port);
                    self.manage_task(ManagedTaskAdd::ignore(
                        handle,
                        &format!("admin interface, port {}", port),
                    ))
                    .await?
                }
                for p in ports {
                    self.add_admin_port(p);
                }
            }
            Ok(())
        }

        pub(crate) async fn add_app_interface(
            self: Arc<Self>,
            port: either::Either<u16, AppInterfaceId>,
        ) -> ConductorResult<u16> {
            let interface_id = match port {
                either::Either::Left(port) => AppInterfaceId::new(port),
                either::Either::Right(id) => id,
            };
            let port = interface_id.port();
            tracing::debug!("Attaching interface {}", port);
            let app_api = RealAppInterfaceApi::new(self.clone());
            // This receiver is thrown away because we can produce infinite new
            // receivers from the Sender
            let (signal_tx, _r) = tokio::sync::broadcast::channel(SIGNAL_BUFFER_SIZE);
            let stop_rx = self.task_manager.share_ref(|tm| {
                tm.as_ref()
                    .expect("Task manager not initialized")
                    .task_stop_broadcaster()
                    .subscribe()
            });
            let (port, task) = spawn_app_interface_task(port, app_api, signal_tx.clone(), stop_rx)
                .await
                .map_err(Box::new)?;
            // TODO: RELIABILITY: Handle this task by restarting it if it fails and log the error
            self.manage_task(ManagedTaskAdd::ignore(
                task,
                &format!("app interface, port {}", port),
            ))
            .await?;
            let interface = AppInterfaceRuntime::Websocket { signal_tx };

            self.app_interfaces.share_mut(|app_interfaces| {
                if app_interfaces.contains_key(&interface_id) {
                    return Err(ConductorError::AppInterfaceIdCollision(
                        interface_id.clone(),
                    ));
                }

                app_interfaces.insert(interface_id.clone(), interface);
                Ok(())
            })?;
            let config = AppInterfaceConfig::websocket(port);
            self.update_state(|mut state| {
                state.app_interfaces.insert(interface_id, config);
                Ok(state)
            })
            .await?;
            tracing::debug!("App interface added at port: {}", port);
            Ok(port)
        }

        /// Returns a port which is guaranteed to have a websocket listener with an Admin interface
        /// on it. Useful for specifying port 0 and letting the OS choose a free port.
        pub fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
            self.admin_websocket_ports.share_ref(|p| p.get(0).copied())
        }

        pub(crate) async fn list_app_interfaces(&self) -> ConductorResult<Vec<u16>> {
            Ok(self
                .get_state()
                .await?
                .app_interfaces
                .values()
                .map(|config| config.driver.port())
                .collect())
        }

        /// Start all app interfaces currently in state.
        /// This should only be run at conductor initialization.
        #[allow(irrefutable_let_patterns)]
        pub(crate) async fn startup_app_interfaces(self: Arc<Self>) -> ConductorResult<()> {
            for id in self.get_state().await?.app_interfaces.keys().cloned() {
                tracing::debug!("Starting up app interface: {:?}", id);
                let _ = self.clone().add_app_interface(either::Right(id)).await?;
            }
            Ok(())
        }
    }
}

/// DNA-related methods
mod dna_impls {

    use super::*;

    impl Conductor {
        /// Get the list of hashes of installed Dnas in this Conductor
        pub fn list_dnas(&self) -> Vec<DnaHash> {
            self.ribosome_store().share_ref(|ds| ds.list())
        }

        /// Get a [`DnaDef`](holochain_types::prelude::DnaDef) from the [`RibosomeStore`](crate::conductor::ribosome_store::RibosomeStore)
        pub fn get_dna_def(&self, hash: &DnaHash) -> Option<DnaDef> {
            self.ribosome_store().share_ref(|ds| ds.get_dna_def(hash))
        }

        /// Get a [`DnaFile`](holochain_types::dna::DnaFile) from the [`RibosomeStore`](crate::conductor::ribosome_store::RibosomeStore)
        pub fn get_dna_file(&self, hash: &DnaHash) -> Option<DnaFile> {
            self.ribosome_store().share_ref(|ds| ds.get_dna_file(hash))
        }

        /// Get an [`EntryDef`](holochain_zome_types::EntryDef) from the [`EntryDefBufferKey`](holochain_types::dna::EntryDefBufferKey)
        pub fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
            self.ribosome_store().share_ref(|ds| ds.get_entry_def(key))
        }

        pub(crate) async fn register_dna_wasm(
            &self,
            ribosome: RealRibosome,
        ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
            let is_full_wasm_dna = ribosome
                .dna_def()
                .all_zomes()
                .all(|(_, zome_def)| matches!(zome_def, ZomeDef::Wasm(_)));

            // Only install wasm if the DNA is composed purely of WasmZomes (no InlineZomes)
            if is_full_wasm_dna {
                Ok(self.put_wasm(ribosome).await?)
            } else {
                Ok(Vec::with_capacity(0))
            }
        }

        pub(crate) fn register_dna_entry_defs(
            &self,
            entry_defs: Vec<(EntryDefBufferKey, EntryDef)>,
        ) {
            self.ribosome_store
                .share_mut(|d| d.add_entry_defs(entry_defs));
        }

        pub(crate) fn add_ribosome_to_store(&self, ribosome: RealRibosome) {
            self.ribosome_store.share_mut(|d| d.add_ribosome(ribosome));
        }

        pub(crate) async fn load_wasms_into_dna_files(
            &self,
        ) -> ConductorResult<(
            impl IntoIterator<Item = (DnaHash, RealRibosome)>,
            impl IntoIterator<Item = (EntryDefBufferKey, EntryDef)>,
        )> {
            let db = &self.spaces.wasm_db;

            // Load out all dna defs
            let (wasm_tasks, defs) = db
                .async_reader(move |txn| {
                    // Get all the dna defs.
                    let dna_defs: Vec<_> = holochain_state::dna_def::get_all(&txn)?
                        .into_iter()
                        .collect();

                    // Gather all the unique wasms.
                    let unique_wasms = dna_defs
                        .iter()
                        .flat_map(|dna_def| {
                            dna_def
                                .all_zomes()
                                .map(|(zome_name, zome)| Ok(zome.wasm_hash(zome_name)?))
                        })
                        .collect::<ConductorResult<HashSet<_>>>()?;

                    // Get the code for each unique wasm.
                    let wasms = unique_wasms
                        .into_iter()
                        .map(|wasm_hash| {
                            holochain_state::wasm::get(&txn, &wasm_hash)?
                                .map(|hashed| hashed.into_content())
                                .ok_or(ConductorError::WasmMissing)
                                .map(|wasm| (wasm_hash, wasm))
                        })
                        .collect::<ConductorResult<HashMap<_, _>>>()?;
                    let wasm_tasks = holochain_state::dna_def::get_all(&txn)?
                        .into_iter()
                        .map(|dna_def| {
                            // Load all wasms for each dna_def from the wasm db into memory
                            let wasms = dna_def.all_zomes().filter_map(|(zome_name, zome)| {
                                let wasm_hash = zome.wasm_hash(zome_name).ok()?;
                                // Note this is a cheap arc clone.
                                wasms.get(&wasm_hash).cloned()
                            });
                            let wasms = wasms.collect::<Vec<_>>();
                            async move {
                                let dna_file = DnaFile::new(dna_def.into_content(), wasms).await;
                                let ribosome = RealRibosome::new(dna_file)?;
                                ConductorResult::Ok((ribosome.dna_hash().clone(), ribosome))
                            }
                        })
                        // This needs to happen due to the environment not being Send
                        .collect::<Vec<_>>();
                    let defs = holochain_state::entry_def::get_all(&txn)?;
                    ConductorResult::Ok((wasm_tasks, defs))
                })
                .await?;
            // try to join all the tasks and return the list of dna files
            let dnas = futures::future::try_join_all(wasm_tasks).await?;
            Ok((dnas, defs))
        }

        /// Get the root environment directory.
        pub fn root_db_dir(&self) -> &DatabaseRootPath {
            &self.spaces.db_dir
        }

        /// Get the keystore.
        pub fn keystore(&self) -> &MetaLairClient {
            &self.keystore
        }

        /// Get a reference to the conductor's HolochainP2p.
        pub fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef {
            &self.holochain_p2p
        }

        /// Remove cells from the cell map in the Conductor
        pub(crate) async fn remove_cells(&self, cell_ids: &[CellId]) {
            let to_cleanup: Vec<_> = self.cells.share_mut(|cells| {
                cell_ids
                    .iter()
                    .filter_map(|cell_id| cells.remove(cell_id).map(|c| (cell_id, c)))
                    .collect()
            });
            for (cell_id, item) in to_cleanup {
                if let Err(err) = item.cell.cleanup().await {
                    tracing::error!("Error cleaning up Cell: {:?}\nCellId: {}", err, cell_id);
                }
            }
        }

        /// Restart every paused app
        pub(crate) async fn start_paused_apps(&self) -> ConductorResult<AppStatusFx> {
            let (_, delta) = self
                .update_state_prime(|mut state| {
                    let ids = state.paused_apps().map(first).cloned().collect::<Vec<_>>();
                    if !ids.is_empty() {
                        tracing::info!("Restarting {} paused apps: {:#?}", ids.len(), ids);
                    }
                    let deltas: Vec<AppStatusFx> = ids
                        .into_iter()
                        .map(|id| {
                            state
                                .transition_app_status(&id, AppStatusTransition::Start)
                                .map(second)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let delta = deltas
                        .into_iter()
                        .fold(AppStatusFx::default(), AppStatusFx::combine);
                    Ok((state, delta))
                })
                .await?;
            Ok(delta)
        }

        pub(crate) async fn put_wasm(
            &self,
            ribosome: RealRibosome,
        ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
            let dna_def = ribosome.dna_def().clone();
            let code = ribosome
                .dna_file()
                .code()
                .clone()
                .into_iter()
                .map(|(_, c)| c);
            let zome_defs = get_entry_defs(ribosome).await?;
            self.put_wasm_code(dna_def, code, zome_defs).await
        }

        pub(crate) async fn put_wasm_code(
            &self,
            dna: DnaDefHashed,
            code: impl Iterator<Item = wasm::DnaWasm>,
            zome_defs: Vec<(EntryDefBufferKey, EntryDef)>,
        ) -> ConductorResult<Vec<(EntryDefBufferKey, EntryDef)>> {
            // TODO: PERF: This loop might be slow
            let wasms = futures::future::join_all(code.map(DnaWasmHashed::from_content)).await;

            self.spaces
                .wasm_db
                .async_commit({
                    let zome_defs = zome_defs.clone();
                    move |txn| {
                        for dna_wasm in wasms {
                            if !holochain_state::wasm::contains(txn, dna_wasm.as_hash())? {
                                holochain_state::wasm::put(txn, dna_wasm)?;
                            }
                        }

                        for (key, entry_def) in zome_defs.clone() {
                            holochain_state::entry_def::put(txn, key, &entry_def)?;
                        }

                        if !holochain_state::dna_def::contains(txn, dna.as_hash())? {
                            holochain_state::dna_def::put(txn, dna.into_content())?;
                        }
                        StateMutationResult::Ok(())
                    }
                })
                .await?;

            Ok(zome_defs)
        }

        pub(crate) async fn load_dnas(&self) -> ConductorResult<()> {
            let (ribosomes, entry_defs) = self.load_wasms_into_dna_files().await?;
            self.ribosome_store().share_mut(|ds| {
                ds.add_ribosomes(ribosomes);
                ds.add_entry_defs(entry_defs);
            });
            Ok(())
        }

        /// Install a [`DnaFile`](holochain_types::dna::DnaFile) in this Conductor
        pub async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()> {
            let ribosome = RealRibosome::new(dna)?;
            let entry_defs = self.register_dna_wasm(ribosome.clone()).await?;
            self.register_dna_entry_defs(entry_defs);
            self.add_ribosome_to_store(ribosome);
            Ok(())
        }
    }
}

/// Network-related methods
mod network_impls {
    use holochain_conductor_api::DnaGossipInfo;
    use holochain_p2p::HolochainP2pSender;

    use super::*;

    impl Conductor {
        /// Get signed agent info from the conductor
        pub async fn get_agent_infos(
            &self,
            cell_id: Option<CellId>,
        ) -> ConductorApiResult<Vec<AgentInfoSigned>> {
            match cell_id {
                Some(c) => {
                    let (d, a) = c.into_dna_and_agent();
                    let db = self.p2p_agents_db(&d);
                    Ok(get_single_agent_info(db.into(), d, a)
                        .await?
                        .map(|a| vec![a])
                        .unwrap_or_default())
                }
                None => {
                    let mut out = Vec::new();
                    // collecting so the mutex lock can close
                    let envs = self.spaces.get_from_spaces(|s| s.p2p_agents_db.clone());
                    for db in envs {
                        out.append(&mut all_agent_infos(db.into()).await?);
                    }
                    Ok(out)
                }
            }
        }

        pub(crate) async fn prune_p2p_agents_db(&self) -> ConductorResult<()> {
            use holochain_p2p::AgentPubKeyExt;

            let mut space_to_agents = HashMap::new();

            for cell in self.cells.share_ref(|c| {
                <Result<_, one_err::OneErr>>::Ok(c.keys().cloned().collect::<Vec<_>>())
            })? {
                space_to_agents
                    .entry(cell.dna_hash().clone())
                    .or_insert_with(Vec::new)
                    .push(cell.agent_pubkey().to_kitsune());
            }

            for (space, agents) in space_to_agents {
                let db = self.spaces.p2p_agents_db(&space)?;
                p2p_prune(&db, agents).await?;
            }

            Ok(())
        }

        pub(crate) async fn gossip_info(
            &self,
            dnas: &[DnaHash],
        ) -> ConductorResult<Vec<DnaGossipInfo>> {
            futures::future::join_all(dnas.iter().map(|dna| async move {
                let m = self
                    .holochain_p2p()
                    .get_diagnostics(dna.clone())
                    .await?
                    .metrics;
                let total_historical_gossip_throughput =
                    m.read().total_current_historical_throughput().into();
                ConductorResult::Ok(DnaGossipInfo {
                    total_historical_gossip_throughput,
                })
            }))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
        }

        #[instrument(skip(self))]
        pub(crate) async fn dispatch_holochain_p2p_event(
            &self,
            event: holochain_p2p::event::HolochainP2pEvent,
        ) -> ConductorApiResult<()> {
            use HolochainP2pEvent::*;
            let dna_hash = event.dna_hash().clone();
            trace!(dispatch_event = ?event);
            match event {
                PutAgentInfoSigned {
                    peer_data, respond, ..
                } => {
                    let sender = self.p2p_batch_sender(&dna_hash);
                    let (result_sender, response) = tokio::sync::oneshot::channel();
                    let _ = sender
                        .send(P2pBatch {
                            peer_data,
                            result_sender,
                        })
                        .await;
                    let res = match response.await {
                        Ok(r) => r.map_err(holochain_p2p::HolochainP2pError::other),
                        Err(e) => Err(holochain_p2p::HolochainP2pError::other(e)),
                    };
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                QueryAgentInfoSigned {
                    kitsune_space,
                    agents,
                    respond,
                    ..
                } => {
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let res = list_all_agent_info(db.into(), kitsune_space)
                        .await
                        .map(|infos| match agents {
                            Some(agents) => infos
                                .into_iter()
                                .filter(|info| agents.contains(&info.agent))
                                .collect(),
                            None => infos,
                        })
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                QueryGossipAgents {
                    since_ms,
                    until_ms,
                    arc_set,
                    respond,
                    ..
                } => {
                    use holochain_sqlite::db::AsP2pAgentStoreConExt;
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let permit = db.conn_permit().await;
                    let res = tokio::task::spawn_blocking(move || {
                        let mut conn = db.with_permit(permit)?;
                        conn.p2p_gossip_query_agents(since_ms, until_ms, (*arc_set).clone())
                    })
                    .await;
                    let res = res
                        .map_err(holochain_p2p::HolochainP2pError::other)
                        .and_then(|r| r.map_err(holochain_p2p::HolochainP2pError::other));
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                QueryAgentInfoSignedNearBasis {
                    kitsune_space,
                    basis_loc,
                    limit,
                    respond,
                    ..
                } => {
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let res = list_all_agent_info_signed_near_basis(
                        db.into(),
                        kitsune_space,
                        basis_loc,
                        limit,
                    )
                    .await
                    .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                QueryPeerDensity {
                    kitsune_space,
                    dht_arc,
                    respond,
                    ..
                } => {
                    let cutoff = self
                        .get_config()
                        .network
                        .clone()
                        .unwrap_or_default()
                        .tuning_params
                        .danger_gossip_recent_threshold();
                    let topo = self
                        .get_dna_def(&dna_hash)
                        .ok_or_else(|| DnaError::DnaMissing(dna_hash.clone()))?
                        .topology(cutoff);
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let res = query_peer_density(db.into(), topo, kitsune_space, dht_arc)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);
                    respond.respond(Ok(async move { res }.boxed().into()));
                }
                SignNetworkData {
                    respond,
                    to_agent,
                    data,
                    ..
                } => {
                    let signature = to_agent.sign_raw(self.keystore(), data.into()).await?;
                    respond.respond(Ok(async move { Ok(signature) }.boxed().into()));
                }
                HolochainP2pEvent::CallRemote { .. }
                | CountersigningSessionNegotiation { .. }
                | Get { .. }
                | GetMeta { .. }
                | GetLinks { .. }
                | GetAgentActivity { .. }
                | MustGetAgentActivity { .. }
                | ValidationReceiptReceived { .. } => {
                    let cell_id =
                        CellId::new(event.dna_hash().clone(), event.target_agents().clone());
                    let cell = self.cell_by_id(&cell_id)?;
                    cell.handle_holochain_p2p_event(event).await?;
                }
                Publish {
                    dna_hash,
                    respond,
                    request_validation_receipt,
                    countersigning_session,
                    ops,
                    ..
                } => {
                    async {
                        let res = self
                            .spaces
                            .handle_publish(
                                &dna_hash,
                                request_validation_receipt,
                                countersigning_session,
                                ops,
                            )
                            .await
                            .map_err(holochain_p2p::HolochainP2pError::other);
                        respond.respond(Ok(async move { res }.boxed().into()));
                    }
                    .instrument(debug_span!("handle_publish"))
                    .await;
                }
                FetchOpData {
                    respond,
                    query,
                    dna_hash,
                    ..
                } => {
                    async {
                        let res = self
                            .spaces
                            .handle_fetch_op_data(&dna_hash, query)
                            .await
                            .map_err(holochain_p2p::HolochainP2pError::other);
                        respond.respond(Ok(async move { res }.boxed().into()));
                    }
                    .instrument(debug_span!("handle_fetch_op_data"))
                    .await;
                }

                HolochainP2pEvent::QueryOpHashes {
                    dna_hash,
                    window,
                    max_ops,
                    include_limbo,
                    arc_set,
                    respond,
                    ..
                } => {
                    let res = self
                        .spaces
                        .handle_query_op_hashes(&dna_hash, arc_set, window, max_ops, include_limbo)
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);

                    respond.respond(Ok(async move { res }.boxed().into()));
                }
            }
            Ok(())
        }

        /// Invoke a zome function on a Cell
        pub async fn call_zome(&self, call: ZomeCall) -> ConductorApiResult<ZomeCallResult> {
            let cell = self.cell_by_id(&call.cell_id)?;
            Ok(cell.call_zome(call, None).await?)
        }

        pub(crate) async fn call_zome_with_workspace(
            &self,
            call: ZomeCall,
            workspace_lock: SourceChainWorkspace,
        ) -> ConductorApiResult<ZomeCallResult> {
            debug!(cell_id = ?call.cell_id);
            let cell = self.cell_by_id(&call.cell_id)?;
            Ok(cell.call_zome(call, Some(workspace_lock)).await?)
        }
    }
}

/// Methods related to app installation and management
mod app_impls {

    use super::*;
    impl Conductor {
        pub(crate) async fn install_app(
            self: Arc<Self>,
            installed_app_id: InstalledAppId,
            cell_data: Vec<(InstalledCell, Option<MembraneProof>)>,
        ) -> ConductorResult<()> {
            crate::conductor::conductor::genesis_cells(
                self.clone(),
                cell_data
                    .iter()
                    .map(|(c, p)| (c.as_id().clone(), p.clone()))
                    .collect(),
            )
            .await?;

            let cell_data = cell_data.into_iter().map(|(c, _)| c);
            let app = InstalledAppCommon::new_legacy(installed_app_id, cell_data)?;

            // Update the db
            let _ = self.add_disabled_app_to_db(app).await?;

            Ok(())
        }

        /// Install DNAs and set up Cells as specified by an AppBundle
        pub async fn install_app_bundle(
            self: Arc<Self>,
            payload: InstallAppBundlePayload,
        ) -> ConductorResult<StoppedApp> {
            let InstallAppBundlePayload {
                source,
                agent_key,
                installed_app_id,
                membrane_proofs,
                network_seed,
            } = payload;

            let bundle: AppBundle = {
                let original_bundle = source.resolve().await?;
                if let Some(network_seed) = network_seed {
                    let mut manifest = original_bundle.manifest().to_owned();
                    manifest.set_network_seed(network_seed);
                    AppBundle::from(original_bundle.into_inner().update_manifest(manifest)?)
                } else {
                    original_bundle
                }
            };

            let installed_app_id =
                installed_app_id.unwrap_or_else(|| bundle.manifest().app_name().to_owned());
            let ops = bundle
                .resolve_cells(agent_key.clone(), DnaGamut::placeholder(), membrane_proofs)
                .await?;

            let cells_to_create = ops.cells_to_create();

            for (dna, _) in ops.dnas_to_register {
                self.clone().register_dna(dna).await?;
            }

            crate::conductor::conductor::genesis_cells(self.clone(), cells_to_create).await?;

            let roles = ops.role_assignments;
            let app = InstalledAppCommon::new(installed_app_id, agent_key, roles)?;

            // Update the db
            let stopped_app = self.add_disabled_app_to_db(app).await?;

            Ok(stopped_app)
        }

        /// Uninstall an app
        #[tracing::instrument(skip(self))]
        pub async fn uninstall_app(
            self: Arc<Self>,
            installed_app_id: &InstalledAppId,
        ) -> ConductorResult<()> {
            let self_clone = self.clone();
            let app = self.remove_app_from_db(installed_app_id).await?;
            tracing::debug!(msg = "Removed app from db.", app = ?app);

            // Remove cells which may now be dangling due to the removed app
            self_clone
                .process_app_status_fx(AppStatusFx::SpinDown, None)
                .await?;
            Ok(())
        }

        /// List active AppIds
        pub async fn list_running_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
            let state = self.get_state().await?;
            Ok(state.running_apps().map(|(id, _)| id).cloned().collect())
        }

        /// List Apps with their information
        pub async fn list_apps(
            &self,
            status_filter: Option<AppStatusFilter>,
        ) -> ConductorResult<Vec<InstalledAppInfo>> {
            use AppStatusFilter::*;
            let conductor_state = self.get_state().await?;

            let apps_ids: Vec<&String> = match status_filter {
                Some(Enabled) => conductor_state.enabled_apps().map(|(id, _)| id).collect(),
                Some(Disabled) => conductor_state.disabled_apps().map(|(id, _)| id).collect(),
                Some(Running) => conductor_state.running_apps().map(|(id, _)| id).collect(),
                Some(Stopped) => conductor_state.stopped_apps().map(|(id, _)| id).collect(),
                Some(Paused) => conductor_state.paused_apps().map(|(id, _)| id).collect(),
                None => conductor_state.installed_apps().keys().collect(),
            };

            let apps_info: Vec<InstalledAppInfo> = apps_ids
                .into_iter()
                .filter_map(|app_id| conductor_state.get_app_info(app_id))
                .collect();

            Ok(apps_info)
        }

        /// Get the IDs of all active installed Apps which use this Cell
        pub async fn list_running_apps_for_dependent_cell_id(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<HashSet<InstalledAppId>> {
            Ok(self
                .get_state()
                .await?
                .running_apps()
                .filter(|(_, v)| v.all_cells().any(|i| i == cell_id))
                .map(|(k, _)| k)
                .cloned()
                .collect())
        }

        /// Find the ID of the first active installed App which uses this Cell
        pub async fn find_cell_with_role_alongside_cell(
            &self,
            cell_id: &CellId,
            role_name: &RoleName,
        ) -> ConductorResult<Option<CellId>> {
            Ok(self
                .get_state()
                .await?
                .running_apps()
                .find(|(_, running_app)| running_app.all_cells().any(|i| i == cell_id))
                .and_then(|(_, running_app)| {
                    running_app
                        .into_common()
                        .role(role_name)
                        .ok()
                        .map(|role| role.cell_id())
                        .cloned()
                }))
        }

        /// Get the IDs of all active installed Apps which use this Dna
        pub async fn list_running_apps_for_dependent_dna_hash(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<HashSet<InstalledAppId>> {
            Ok(self
                .get_state()
                .await?
                .running_apps()
                .filter(|(_, v)| v.all_cells().any(|i| i.dna_hash() == dna_hash))
                .map(|(k, _)| k)
                .cloned()
                .collect())
        }

        /// Get info about an installed App, regardless of status
        pub async fn get_app_info(
            &self,
            installed_app_id: &InstalledAppId,
        ) -> ConductorResult<Option<InstalledAppInfo>> {
            Ok(self.get_state().await?.get_app_info(installed_app_id))
        }
    }
}

/// Methods related to cell access
mod cell_impls {
    use super::*;
    impl Conductor {
        pub(crate) fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<Arc<Cell>> {
            let cell = self
                .cells
                .share_ref(|c| c.get(cell_id).map(|i| i.cell.clone()))
                .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;
            Ok(cell)
        }

        /// Iterator over only the cells which are fully running. Generally used
        /// to handle conductor interface requests
        pub fn running_cell_ids(&self) -> HashSet<CellId> {
            self.cells.share_ref(|c| {
                c.iter()
                    .filter_map(|(id, item)| {
                        if item.is_running() {
                            Some(id.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
        }

        /// List CellIds for Cells which match a status filter
        pub fn list_cell_ids(&self, filter: Option<CellStatusFilter>) -> Vec<CellId> {
            self.cells.share_ref(|cells| {
                cells
                    .iter()
                    .filter_map(|(id, cell)| {
                        let matches = filter
                            .as_ref()
                            .map(|status| cell.status == *status)
                            .unwrap_or(true);
                        if matches {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .cloned()
                    .collect()
            })
        }
    }
}

/// Methods related to clone cell management
mod clone_cell_impls {
    use super::*;

    impl Conductor {
        /// Create a new cell in an existing app based on an existing DNA.
        ///
        /// # Returns
        ///
        /// A struct with the created cell's clone id and cell id.
        pub async fn create_clone_cell(
            self: Arc<Self>,
            payload: CreateCloneCellPayload,
        ) -> ConductorResult<InstalledCell> {
            let CreateCloneCellPayload {
                app_id,
                role_name,
                modifiers,
                membrane_proof,
                name,
            } = payload;
            if !modifiers.has_some_option_set() {
                return Err(ConductorError::CloneCellError(
                    "neither network_seed nor properties nor origin_time provided for clone cell"
                        .to_string(),
                ));
            }
            let state = self.get_state().await?;
            let app = state.get_app(&app_id)?;
            app.provisioned_cells()
                .find(|(app_role_name, _)| **app_role_name == role_name)
                .ok_or_else(|| {
                    ConductorError::CloneCellError(
                        "no base cell found for provided role id".to_string(),
                    )
                })?;

            // add cell to app
            let installed_clone_cell = self
                .add_clone_cell_to_app(
                    app_id.clone(),
                    role_name.clone(),
                    modifiers.serialized()?,
                    name,
                )
                .await?;

            // run genesis on cloned cell
            let cells = vec![(installed_clone_cell.as_id().clone(), membrane_proof)];
            crate::conductor::conductor::genesis_cells(self.clone(), cells).await?;
            self.create_and_add_initialized_cells_for_running_apps(Some(&app_id))
                .await?;
            Ok(installed_clone_cell)
        }

        /// Archive a clone cell for future deletion from the app.
        pub(crate) async fn archive_clone_cell(
            &self,
            ArchiveCloneCellPayload {
                app_id,
                clone_cell_id,
            }: &ArchiveCloneCellPayload,
        ) -> ConductorResult<()> {
            let (_, removed_cell_id) = self
                .update_state_prime({
                    let app_id = app_id.to_owned();
                    let clone_cell_id = clone_cell_id.to_owned();
                    move |mut state| {
                        let app = state.get_app_mut(&app_id)?;
                        let clone_id = app.get_clone_id(&clone_cell_id)?;
                        let cell_id = app.get_clone_cell_id(&clone_cell_id)?;
                        app.archive_clone_cell(&clone_id)?;
                        Ok((state, cell_id))
                    }
                })
                .await?;
            self.remove_cells(&[removed_cell_id]).await;
            Ok(())
        }

        /// Restore an archived clone cell for an app.
        pub(crate) async fn restore_clone_cell(
            &self,
            ArchiveCloneCellPayload {
                app_id,
                clone_cell_id,
            }: &ArchiveCloneCellPayload,
        ) -> ConductorResult<InstalledCell> {
            let (_, restored_cell) = self
                .update_state_prime({
                    let app_id = app_id.to_owned();
                    let clone_cell_id = clone_cell_id.to_owned();
                    move |mut state| {
                        let app = state.get_app_mut(&app_id)?;
                        let clone_id = app.get_archived_clone_id(&clone_cell_id)?;
                        let restored_cell = app.restore_clone_cell(&clone_id)?;
                        Ok((state, restored_cell))
                    }
                })
                .await?;
            Ok(restored_cell)
        }

        /// Remove a clone cell from an app.
        pub(crate) async fn delete_archived_clone_cells(
            &self,
            DeleteArchivedCloneCellsPayload { app_id, role_name }: &DeleteArchivedCloneCellsPayload,
        ) -> ConductorResult<()> {
            self.update_state_prime({
                let app_id = app_id.clone();
                let role_name = role_name.clone();
                move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    app.delete_archived_clone_cells_for_role(&role_name)?;
                    Ok((state, ()))
                }
            })
            .await?;
            self.remove_dangling_cells().await?;
            Ok(())
        }

        /// Restore an archived clone cell
        pub async fn restore_archived_clone_cell(
            self: Arc<Self>,
            payload: &ArchiveCloneCellPayload,
        ) -> ConductorResult<InstalledCell> {
            let restored_cell = self.restore_clone_cell(payload).await?;
            self.create_and_add_initialized_cells_for_running_apps(Some(&payload.app_id))
                .await?;
            Ok(restored_cell)
        }
    }
}

/// Methods related to management of app and cell status
mod app_status_impls {
    use super::*;

    impl Conductor {
        /// Adjust which cells are present in the Conductor (adding and removing as
        /// needed) to match the current reality of all app statuses.
        /// - If a Cell is used by at least one Running app, then ensure it is added
        /// - If a Cell is used by no running apps, then ensure it is removed.
        #[tracing::instrument(skip(self))]
        pub async fn reconcile_cell_status_with_app_status(
            self: Arc<Self>,
        ) -> ConductorResult<CellStartupErrors> {
            self.remove_dangling_cells().await?;

            let results = self
                .create_and_add_initialized_cells_for_running_apps(None)
                .await?;
            Ok(results)
        }

        /// Enable an app
        #[tracing::instrument(skip(self))]
        pub async fn enable_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
        ) -> ConductorResult<(InstalledApp, CellStartupErrors)> {
            let (app, delta) = self
                .transition_app_status(app_id.clone(), AppStatusTransition::Enable)
                .await?;
            let errors = self
                .process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
                .await?;
            Ok((app, errors))
        }

        /// Disable an app
        #[tracing::instrument(skip(self))]
        pub async fn disable_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
            reason: DisabledAppReason,
        ) -> ConductorResult<InstalledApp> {
            let (app, delta) = self
                .transition_app_status(app_id.clone(), AppStatusTransition::Disable(reason))
                .await?;
            self.process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
                .await?;
            Ok(app)
        }

        /// Start an app
        #[tracing::instrument(skip(self))]
        pub async fn start_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
        ) -> ConductorResult<InstalledApp> {
            let (app, delta) = self
                .transition_app_status(app_id.clone(), AppStatusTransition::Start)
                .await?;
            self.process_app_status_fx(delta, Some(vec![app_id.to_owned()].into_iter().collect()))
                .await?;
            Ok(app)
        }

        /// Register an app as disabled in the database
        pub(crate) async fn add_disabled_app_to_db(
            &self,
            app: InstalledAppCommon,
        ) -> ConductorResult<StoppedApp> {
            let (_, stopped_app) = self
                .update_state_prime(move |mut state| {
                    let stopped_app = state.add_app(app)?;
                    Ok((state, stopped_app))
                })
                .await?;
            Ok(stopped_app)
        }

        /// Transition an app's status to a new state.
        #[tracing::instrument(skip(self))]
        pub(crate) async fn transition_app_status(
            &self,
            app_id: InstalledAppId,
            transition: AppStatusTransition,
        ) -> ConductorResult<(InstalledApp, AppStatusFx)> {
            Ok(self
                .update_state_prime(move |mut state| {
                    let (app, delta) = state.transition_app_status(&app_id, transition)?.clone();
                    let app = app.clone();
                    Ok((state, (app, delta)))
                })
                .await?
                .1)
        }

        /// Pause an app
        #[tracing::instrument(skip(self))]
        #[cfg(any(test, feature = "test_utils"))]
        pub async fn pause_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
            reason: PausedAppReason,
        ) -> ConductorResult<InstalledApp> {
            let (app, delta) = self
                .transition_app_status(app_id.clone(), AppStatusTransition::Pause(reason))
                .await?;
            self.process_app_status_fx(delta, Some(vec![app_id.clone()].into_iter().collect()))
                .await?;
            Ok(app)
        }

        /// Create any Cells which are missing for any running apps, then initialize
        /// and join them. (Joining could take a while.)
        pub(crate) async fn create_and_add_initialized_cells_for_running_apps(
            self: Arc<Self>,
            app_id: Option<&InstalledAppId>,
        ) -> ConductorResult<CellStartupErrors> {
            let results = self.clone().create_cells_for_running_apps(app_id).await?;
            let (new_cells, errors): (Vec<_>, Vec<_>) =
                results.into_iter().partition(Result::is_ok);

            let new_cells = new_cells
                .into_iter()
                // We can unwrap the successes because of the partition
                .map(Result::unwrap)
                .collect();

            let errors = errors
                .into_iter()
                // throw away the non-Debug types which will be unwrapped away anyway
                .map(|r| r.map(|_| ()))
                // We can unwrap the errors because of the partition
                .map(Result::unwrap_err)
                .collect();

            // Add the newly created cells to the Conductor with the PendingJoin
            // status, and start their workflow loops
            self.add_and_initialize_cells(new_cells);

            // Join these newly created cells to the network
            // (as well as any others which need joining)
            self.join_all_pending_cells().await;

            Ok(errors)
        }

        /// Attempt to join all PendingJoin cells to the kitsune network.
        /// Returns the cells which were joined during this call.
        ///
        /// NB: this could take as long as JOIN_NETWORK_TIMEOUT, which is significant.
        ///   Be careful to only await this future if it's important that cells be
        ///   joined before proceeding.
        pub(crate) async fn join_all_pending_cells(&self) -> Vec<CellId> {
            // Join the network but ignore errors because the
            // space retries joining all cells every 5 minutes.

            use holochain_p2p::AgentPubKeyExt;

            let tasks = self
            .mark_pending_cells_as_joining()
            .into_iter()
            .map(|(cell_id, cell)| async move {
                let p2p_agents_db = cell.p2p_agents_db().clone();
                let kagent = cell_id.agent_pubkey().to_kitsune();
                let agent_info = match p2p_agents_db.async_reader(move |tx| {
                    tx.p2p_get_agent(&kagent)
                }).await {
                    Ok(maybe_info) => maybe_info,
                    _ => None,
                };
                let maybe_initial_arc = agent_info.map(|i| i.storage_arc);
                let network = cell.holochain_p2p_dna().clone();
                match tokio::time::timeout(JOIN_NETWORK_TIMEOUT, network.join(cell_id.agent_pubkey().clone(), maybe_initial_arc)).await {
                    Ok(Err(e)) => {
                        tracing::info!(error = ?e, cell_id = ?cell_id, "Error while trying to join the network");
                        Err(cell_id)
                    }
                    Err(_) => {
                        tracing::info!(cell_id = ?cell_id, "Timed out trying to join the network");
                        Err(cell_id)
                    }
                    Ok(Ok(_)) => Ok(cell_id),
                }
            });

            let maybes: Vec<_> = futures::stream::iter(tasks)
                .buffer_unordered(100)
                .collect()
                .await;

            let (cell_ids, failed_joins): (Vec<_>, Vec<_>) =
                maybes.into_iter().partition(Result::is_ok);

            // These unwraps are both safe because of the partition.
            let cell_ids: Vec<_> = cell_ids.into_iter().map(Result::unwrap).collect();
            let failed_joins: Vec<_> = failed_joins.into_iter().map(Result::unwrap_err).collect();

            // Update the status of the cells which were able to join the network
            // (may or may not be all cells which were added)
            self.update_cell_status(cell_ids.as_slice(), CellStatus::Joined);

            self.update_cell_status(failed_joins.as_slice(), CellStatus::PendingJoin);

            cell_ids
        }

        /// Adjust app statuses (via state transitions) to match the current
        /// reality of which Cells are present in the conductor.
        /// - Do not change state for Disabled apps. For all others:
        /// - If an app is Paused but all of its (required) Cells are on,
        ///     then set it to Running
        /// - If an app is Running but at least one of its (required) Cells are off,
        ///     then set it to Paused
        pub(crate) async fn reconcile_app_status_with_cell_status(
            &self,
            app_ids: Option<HashSet<InstalledAppId>>,
        ) -> ConductorResult<AppStatusFx> {
            use AppStatus::*;
            use AppStatusTransition::*;

            let running_cells: HashSet<CellId> = self.running_cell_ids();
            let (_, delta) = self
                .update_state_prime(move |mut state| {
                    #[allow(deprecated)]
                    let apps = state.installed_apps_mut().iter_mut().filter(|(id, _)| {
                        app_ids
                            .as_ref()
                            .map(|ids| ids.contains(&**id))
                            .unwrap_or(true)
                    });
                    let delta = apps
                        .into_iter()
                        .map(|(_app_id, app)| {
                            match app.status().clone() {
                                Running => {
                                    // If not all required cells are running, pause the app
                                    let missing: Vec<_> = app
                                        .required_cells()
                                        .filter(|id| !running_cells.contains(id))
                                        .collect();
                                    if !missing.is_empty() {
                                        let reason = PausedAppReason::Error(format!(
                                            "Some cells are missing / not able to run: {:#?}",
                                            missing
                                        ));
                                        app.status.transition(Pause(reason))
                                    } else {
                                        AppStatusFx::NoChange
                                    }
                                }
                                Paused(_) => {
                                    // If all required cells are now running, restart the app
                                    if app.required_cells().all(|id| running_cells.contains(id)) {
                                        app.status.transition(Start)
                                    } else {
                                        AppStatusFx::NoChange
                                    }
                                }
                                Disabled(_) => {
                                    // Disabled status should never automatically change.
                                    AppStatusFx::NoChange
                                }
                            }
                        })
                        .fold(AppStatusFx::default(), AppStatusFx::combine);
                    Ok((state, delta))
                })
                .await?;
            Ok(delta)
        }

        /// Change the CellStatus of the given Cells in the Conductor.
        /// Silently ignores Cells that don't exist.
        pub(crate) fn update_cell_status(&self, cell_ids: &[CellId], status: CellStatus) {
            for cell_id in cell_ids {
                self.cells.share_mut(|cells| {
                    if let Some(mut cell) = cells.get_mut(cell_id) {
                        cell.status = status.clone();
                    }
                });
            }
        }
    }
}

/// Methods related to management of Conductor state
mod state_impls {
    use super::*;

    impl Conductor {
        pub(crate) async fn get_state(&self) -> ConductorResult<ConductorState> {
            self.spaces.get_state().await
        }

        /// Update the internal state with a pure function mapping old state to new
        pub(crate) async fn update_state<F: Send>(&self, f: F) -> ConductorResult<ConductorState>
        where
            F: FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
        {
            self.spaces.update_state(f).await
        }

        /// Update the internal state with a pure function mapping old state to new,
        /// which may also produce an output value which will be the output of
        /// this function
        pub(crate) async fn update_state_prime<F, O>(
            &self,
            f: F,
        ) -> ConductorResult<(ConductorState, O)>
        where
            F: FnOnce(ConductorState) -> ConductorResult<(ConductorState, O)> + Send + 'static,
            O: Send + 'static,
        {
            self.check_running()?;
            self.spaces.update_state_prime(f).await
        }

        /// Sends a JoinHandle to the TaskManager task to be managed
        pub(crate) async fn manage_task(&self, handle: ManagedTaskAdd) -> ConductorResult<()> {
            self.task_manager
                .share_ref(|tm| {
                    tm.as_ref()
                        .expect("Task manager not initialized")
                        .task_add_sender()
                        .clone()
                })
                .send(handle)
                .await
                .map_err(|e| ConductorError::SubmitTaskError(format!("{}", e)))
        }
    }
}

/// Methods related to zome function scheduling
mod scheduler_impls {
    use super::*;

    impl Conductor {
        pub(super) fn set_scheduler(&self, join_handle: tokio::task::JoinHandle<()>) {
            let mut scheduler = self.scheduler.lock();
            if let Some(existing_join_handle) = &*scheduler {
                existing_join_handle.abort();
            }
            *scheduler = Some(join_handle);
        }

        /// Start the scheduler. None is not an option.
        /// Calling this will:
        /// - Delete/unschedule all ephemeral scheduled functions GLOBALLY
        /// - Add an interval that runs IN ADDITION to previous invocations
        /// So ideally this would be called ONCE per conductor lifecyle ONLY.
        pub(crate) async fn start_scheduler(self: Arc<Self>, interval_period: std::time::Duration) {
            // Clear all ephemeral cruft in all cells before starting a scheduler.
            let cell_arcs = {
                let mut cell_arcs = vec![];
                for cell_id in self.running_cell_ids() {
                    if let Ok(cell_arc) = self.cell_by_id(&cell_id) {
                        cell_arcs.push(cell_arc);
                    }
                }
                cell_arcs
            };
            let tasks = cell_arcs
                .into_iter()
                .map(|cell_arc| cell_arc.delete_all_ephemeral_scheduled_fns());
            futures::future::join_all(tasks).await;

            let scheduler_handle = self.clone();
            self.set_scheduler(tokio::task::spawn(async move {
                let mut interval = tokio::time::interval(interval_period);
                loop {
                    interval.tick().await;
                    scheduler_handle
                        .clone()
                        .dispatch_scheduled_fns(Timestamp::now())
                        .await;
                }
            }));
        }

        /// The scheduler wants to dispatch any functions that are due.
        pub(crate) async fn dispatch_scheduled_fns(self: Arc<Self>, now: Timestamp) {
            let cell_arcs = {
                let mut cell_arcs = vec![];
                for cell_id in self.running_cell_ids() {
                    if let Ok(cell_arc) = self.cell_by_id(&cell_id) {
                        cell_arcs.push(cell_arc);
                    }
                }
                cell_arcs
            };

            let tasks = cell_arcs
                .into_iter()
                .map(|cell_arc| cell_arc.dispatch_scheduled_fns(now));
            futures::future::join_all(tasks).await;
        }
    }
}

/// Miscellaneous methods
mod misc_impls {
    use holochain_zome_types::builder;

    use super::*;

    impl Conductor {
        /// Grant a zome call capability for a cell
        pub async fn grant_zome_call_capability(
            &self,
            payload: GrantZomeCallCapabilityPayload,
        ) -> ConductorApiResult<()> {
            let GrantZomeCallCapabilityPayload { cell_id, cap_grant } = payload;

            let source_chain = SourceChain::new(
                self.get_authored_db(cell_id.dna_hash())?,
                self.get_dht_db(cell_id.dna_hash())?,
                self.get_dht_db_cache(cell_id.dna_hash())?,
                self.keystore.clone(),
                cell_id.agent_pubkey().clone(),
            )
            .await?;

            let cap_grant_entry = Entry::CapGrant(cap_grant);
            let entry_hash = EntryHash::with_data_sync(&cap_grant_entry);
            let action_builder = builder::Create {
                entry_type: EntryType::CapGrant,
                entry_hash,
            };

            source_chain
                .put_weightless(
                    action_builder,
                    Some(cap_grant_entry),
                    ChainTopOrdering::default(),
                )
                .await?;

            let cell = self.cell_by_id(&cell_id)?;
            source_chain.flush(cell.holochain_p2p_dna()).await?;

            Ok(())
        }

        /// Create a JSON dump of the cell's state
        pub async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
            let cell = self.cell_by_id(cell_id)?;
            let authored_db = cell.authored_db();
            let dht_db = cell.dht_db();
            let space = cell_id.dna_hash();
            let p2p_agents_db = self.p2p_agents_db(space);

            let peer_dump =
                p2p_agent_store::dump_state(p2p_agents_db.into(), Some(cell_id.clone())).await?;
            let source_chain_dump = source_chain::dump_state(
                authored_db.clone().into(),
                cell_id.agent_pubkey().clone(),
            )
            .await?;

            let out = JsonDump {
                peer_dump,
                source_chain_dump,
                integration_dump: integration_dump(&dht_db.clone().into()).await?,
            };
            // Add summary
            let summary = out.to_string();
            let out = (out, summary);
            Ok(serde_json::to_string_pretty(&out)?)
        }

        /// Create a comprehensive structured dump of a cell's state
        pub async fn dump_full_cell_state(
            &self,
            cell_id: &CellId,
            dht_ops_cursor: Option<u64>,
        ) -> ConductorApiResult<FullStateDump> {
            let authored_db = self.get_or_create_authored_db(cell_id.dna_hash())?;
            let dht_db = self.get_or_create_dht_db(cell_id.dna_hash())?;
            let dna_hash = cell_id.dna_hash();
            let p2p_agents_db = self.spaces.p2p_agents_db(dna_hash)?;

            let peer_dump =
                p2p_agent_store::dump_state(p2p_agents_db.into(), Some(cell_id.clone())).await?;
            let source_chain_dump =
                source_chain::dump_state(authored_db.into(), cell_id.agent_pubkey().clone())
                    .await?;

            let out = FullStateDump {
                peer_dump,
                source_chain_dump,
                integration_dump: full_integration_dump(&dht_db, dht_ops_cursor).await?,
            };
            Ok(out)
        }

        /// JSON dump of network metrics
        pub async fn dump_network_metrics(
            &self,
            dna_hash: Option<DnaHash>,
        ) -> ConductorApiResult<String> {
            use holochain_p2p::HolochainP2pSender;
            self.holochain_p2p()
                .dump_network_metrics(dna_hash)
                .await
                .map_err(crate::conductor::api::error::ConductorApiError::other)
        }

        /// Add signed agent info to the conductor
        pub async fn add_agent_infos(
            &self,
            agent_infos: Vec<AgentInfoSigned>,
        ) -> ConductorApiResult<()> {
            let mut space_map = HashMap::new();
            for agent_info_signed in agent_infos {
                let space = agent_info_signed.space.clone();
                space_map
                    .entry(space)
                    .or_insert_with(Vec::new)
                    .push(agent_info_signed);
            }
            for (space, agent_infos) in space_map {
                let db = self.p2p_agents_db(&DnaHash::from_kitsune(&space));
                inject_agent_infos(db, agent_infos.iter()).await?;
            }
            Ok(())
        }

        /// Inject records into a source chain for a cell.
        /// If the records form a chain segment that can be "grafted" onto the existing chain, it will be.
        /// Otherwise, a new chain will be formed using the specified records.
        pub async fn graft_records_onto_source_chain(
            self: Arc<Self>,
            cell_id: CellId,
            validate: bool,
            records: Vec<Record>,
        ) -> ConductorApiResult<()> {
            graft_records_onto_source_chain::graft_records_onto_source_chain(
                self, cell_id, validate, records,
            )
            .await
        }

        /// Update coordinator zomes on an existing dna.
        pub async fn update_coordinators(
            &self,
            hash: &DnaHash,
            coordinator_zomes: CoordinatorZomes,
            wasms: Vec<wasm::DnaWasm>,
        ) -> ConductorResult<()> {
            // Note this isn't really concurrent safe. It would be a race condition to update the
            // same dna concurrently.
            let mut ribosome = self
                .ribosome_store()
                .share_ref(|d| match d.get_ribosome(hash) {
                    Some(dna) => Ok(dna),
                    None => Err(DnaError::DnaMissing(hash.to_owned())),
                })?;
            let _old_wasms = ribosome
                .dna_file
                .update_coordinators(coordinator_zomes.clone(), wasms.clone())
                .await?;

            // Add new wasm code to db.
            self.put_wasm_code(
                ribosome.dna_def().clone(),
                wasms.into_iter(),
                Vec::with_capacity(0),
            )
            .await?;

            // Update RibosomeStore.
            self.ribosome_store()
                .share_mut(|d| d.add_ribosome(ribosome));

            // TODO: Remove old wasm code? (Maybe this needs to be done on restart as it could be in use).

            Ok(())
        }
    }
}

/// Pure accessor methods
mod accessor_impls {
    use super::*;

    impl Conductor {
        pub(crate) fn ribosome_store(&self) -> &RwShare<RibosomeStore> {
            &self.ribosome_store
        }

        pub(crate) fn get_queue_consumer_workflows(&self) -> QueueConsumerMap {
            self.spaces.queue_consumer_map.clone()
        }

        /// Access to the signal broadcast channel, to create
        /// new subscriptions
        pub fn signal_broadcaster(&self) -> SignalBroadcaster {
            let senders = self
                .app_interfaces
                .share_ref(|ai| ai.values().map(|i| i.signal_tx()).cloned().collect());
            SignalBroadcaster::new(senders)
        }

        /// Instantiate a Ribosome for use with a DNA
        pub(crate) fn get_ribosome(&self, dna_hash: &DnaHash) -> ConductorResult<RealRibosome> {
            self.ribosome_store
                .share_ref(|d| match d.get_ribosome(dna_hash) {
                    Some(r) => Ok(r),
                    None => Err(DnaError::DnaMissing(dna_hash.to_owned()).into()),
                })
        }

        /// Get a dna space or create it if one doesn't exist.
        pub(crate) fn get_or_create_space(&self, dna_hash: &DnaHash) -> ConductorResult<Space> {
            self.spaces.get_or_create_space(dna_hash)
        }

        pub(crate) fn get_or_create_authored_db(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<DbWrite<DbKindAuthored>> {
            self.spaces.authored_db(dna_hash)
        }

        pub(crate) fn get_or_create_dht_db(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<DbWrite<DbKindDht>> {
            self.spaces.dht_db(dna_hash)
        }

        pub(crate) fn p2p_agents_db(&self, hash: &DnaHash) -> DbWrite<DbKindP2pAgents> {
            self.spaces
                .p2p_agents_db(hash)
                .expect("failed to open p2p_agent_store database")
        }

        pub(crate) fn p2p_batch_sender(
            &self,
            hash: &DnaHash,
        ) -> tokio::sync::mpsc::Sender<P2pBatch> {
            self.spaces
                .p2p_batch_sender(hash)
                .expect("failed to get p2p_batch_sender")
        }

        pub(crate) fn p2p_metrics_db(&self, hash: &DnaHash) -> DbWrite<DbKindP2pMetrics> {
            self.spaces
                .p2p_metrics_db(hash)
                .expect("failed to open p2p_metrics_store database")
        }

        /// Get the post commit sender.
        pub async fn post_commit_permit(
            &self,
        ) -> Result<tokio::sync::mpsc::OwnedPermit<PostCommitArgs>, SendError<()>> {
            self.post_commit.clone().reserve_owned().await
        }

        /// Get the conductor config
        pub fn get_config(&self) -> &ConductorConfig {
            &self.config
        }
    }
}

/// Private methods, only used within the Conductor, never called from outside.
impl Conductor {
    fn add_admin_port(&self, port: u16) {
        self.admin_websocket_ports.share_mut(|p| p.push(port));
    }

    /// Add fully constructed cells to the cell map in the Conductor
    fn add_and_initialize_cells(&self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (new_cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        self.cells.share_mut(|cells| {
            for cell in new_cells {
                let cell_id = cell.id().clone();
                tracing::debug!(?cell_id, "added cell");
                cells.insert(
                    cell_id,
                    CellItem {
                        cell: Arc::new(cell),
                        status: CellStatus::PendingJoin,
                    },
                );
            }
        });
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }

    /// Return Cells which are pending network join, and mark them as
    /// currently joining.
    ///
    /// Used to discover which cells need to be joined to the network.
    /// The cells' status are upgraded to `Joining` when this function is called.
    fn mark_pending_cells_as_joining(&self) -> Vec<(CellId, Arc<Cell>)> {
        self.cells.share_mut(|cells| {
            cells
                .iter_mut()
                .filter_map(|(id, item)| {
                    if item.is_pending() {
                        item.status = CellStatus::Joining;
                        Some((id.clone(), item.cell.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// Remove all Cells which are not referenced by any Enabled app.
    /// (Cells belonging to Paused apps are not considered "dangling" and will not be removed)
    async fn remove_dangling_cells(&self) -> ConductorResult<()> {
        let state = self.get_state().await?;
        let keepers: HashSet<CellId> = state
            .enabled_apps()
            .flat_map(|(_, app)| app.all_cells().cloned().collect::<HashSet<_>>())
            .collect();

        // Clean up all cells that will be dropped (leave network, etc.)
        let to_cleanup: Vec<_> = self.cells.share_mut(|cells| {
            let to_remove = cells
                .keys()
                .filter(|id| !keepers.contains(id))
                .cloned()
                .collect::<Vec<_>>();

            to_remove
                .iter()
                .filter_map(|cell_id| cells.remove(cell_id))
                .collect()
        });
        for cell in to_cleanup {
            cell.cell.cleanup().await?;
        }

        // drop all but the keepers
        Ok(())
    }

    /// Attempt to create all necessary Cells which have not already been created
    /// and added to the conductor, namely the cells which are referenced by
    /// Running apps. If there are no cells to create, this function does nothing.
    ///
    /// Accepts an optional app id to only create cells of that app instead of all apps.
    ///
    /// Returns a Result for each attempt so that successful creations can be
    /// handled alongside the failures.
    async fn create_cells_for_running_apps(
        self: Arc<Self>,
        app_id: Option<&InstalledAppId>,
    ) -> ConductorResult<Vec<Result<(Cell, InitialQueueTriggers), (CellId, CellError)>>> {
        // Data required to create apps
        let (managed_task_add_sender, managed_task_stop_broadcaster) =
            self.task_manager.share_ref(|tm| {
                let tm = tm.as_ref().expect("Task manager not initialized");
                (
                    tm.task_add_sender().clone(),
                    tm.task_stop_broadcaster().clone(),
                )
            });

        // Closure for creating all cells in an app
        let state = self.get_state().await?;

        let app_cells: HashSet<CellId> = match app_id {
            Some(app_id) => {
                let app = state.get_app(app_id)?;
                if app.status().is_running() {
                    app.all_cells().into_iter().cloned().collect()
                } else {
                    HashSet::new()
                }
            }
            None =>
            // Collect all CellIds across all apps, deduped
            {
                state
                    .installed_apps()
                    .iter()
                    .filter(|(_, app)| app.status().is_running())
                    .flat_map(|(_id, app)| app.all_cells().collect::<Vec<&CellId>>())
                    .cloned()
                    .collect()
            }
        };

        // calculate the existing cells so we can filter those out, only creating
        // cells for CellIds that don't have cells
        let on_cells: HashSet<CellId> = self.cells.share_ref(|c| c.keys().cloned().collect());

        let tasks = app_cells.difference(&on_cells).map(|cell_id| {
            let handle = self.clone();
            let managed_task_add_sender = managed_task_add_sender.clone();
            let managed_task_stop_broadcaster = managed_task_stop_broadcaster.clone();
            let chc = handle.chc(cell_id);
            async move {
                let holochain_p2p_cell =
                    handle.holochain_p2p.to_dna(cell_id.dna_hash().clone(), chc);

                let space = handle
                    .get_or_create_space(cell_id.dna_hash())
                    .map_err(|e| CellError::FailedToCreateDnaSpace(e.into()))
                    .map_err(|err| (cell_id.clone(), err))?;

                Cell::create(
                    cell_id.clone(),
                    handle,
                    space,
                    holochain_p2p_cell,
                    managed_task_add_sender,
                    managed_task_stop_broadcaster,
                )
                .await
                .map_err(|err| (cell_id.clone(), err))
            }
        });

        // Join on all apps and return a list of
        // apps that had succelly created cells
        // and any apps that encounted errors
        Ok(futures::future::join_all(tasks).await)
    }

    /// Deal with the side effects of an app status state transition
    async fn process_app_status_fx(
        self: Arc<Self>,
        delta: AppStatusFx,
        app_ids: Option<HashSet<InstalledAppId>>,
    ) -> ConductorResult<CellStartupErrors> {
        use AppStatusFx::*;
        let mut last = (delta, vec![]);
        loop {
            tracing::debug!(msg = "Processing app status delta", delta = ?last.0);
            last = match last.0 {
                NoChange => break,
                SpinDown => {
                    // Reconcile cell status so that dangling cells can leave the network and be removed
                    let errors = self.clone().reconcile_cell_status_with_app_status().await?;

                    // TODO: This should probably be emitted over the admin interface
                    if !errors.is_empty() {
                        error!(msg = "Errors when trying to stop app(s)", ?errors);
                    }

                    (NoChange, errors)
                }
                SpinUp | Both => {
                    // Reconcile cell status so that missing/pending cells can become fully joined
                    let errors = self.clone().reconcile_cell_status_with_app_status().await?;

                    // Reconcile app status in case some cells failed to join, so the app can be paused
                    let delta = self
                        .clone()
                        .reconcile_app_status_with_cell_status(app_ids.clone())
                        .await?;

                    // TODO: This should probably be emitted over the admin interface
                    if !errors.is_empty() {
                        error!(msg = "Errors when trying to start app(s)", ?errors);
                    }

                    (delta, errors)
                }
            };
        }

        Ok(last.1)
    }

    /// Entirely remove an app from the database, returning the removed app.
    async fn remove_app_from_db(&self, app_id: &InstalledAppId) -> ConductorResult<InstalledApp> {
        let (_state, app) = self
            .update_state_prime({
                let app_id = app_id.clone();
                move |mut state| {
                    let app = state.remove_app(&app_id)?;
                    Ok((state, app))
                }
            })
            .await?;
        Ok(app)
    }

    /// Associate a new clone cell with an existing app.
    async fn add_clone_cell_to_app(
        &self,
        app_id: InstalledAppId,
        role_name: RoleName,
        dna_modifiers: DnaModifiersOpt,
        name: Option<String>,
    ) -> ConductorResult<InstalledCell> {
        let ribosome_store = &self.ribosome_store;
        // retrieve base cell DNA hash from conductor
        let (_, base_cell_dna_hash) = self
            .update_state_prime({
                let app_id = app_id.clone();
                let role_name = role_name.clone();
                move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    let app_role_assignment = app
                        .roles()
                        .get(&role_name)
                        .ok_or_else(|| AppError::RoleNameMissing(role_name.to_owned()))?;
                    if app_role_assignment.is_clone_limit_reached() {
                        return Err(ConductorError::AppError(AppError::CloneLimitExceeded(
                            app_role_assignment.clone_limit(),
                            app_role_assignment.clone(),
                        )));
                    }
                    let parent_dna_hash = app_role_assignment.dna_hash().clone();
                    Ok((state, parent_dna_hash))
                }
            })
            .await?;
        // clone cell from base cell DNA
        let clone_dna = ribosome_store.share_ref(|ds| {
            let mut dna_file = ds
                .get_dna_file(&base_cell_dna_hash)
                .ok_or(DnaError::DnaMissing(base_cell_dna_hash))?
                .update_modifiers(dna_modifiers);
            if let Some(name) = name {
                dna_file = dna_file.set_name(name);
            }
            Ok::<_, DnaError>(dna_file)
        })?;
        let clone_dna_hash = clone_dna.dna_hash().to_owned();
        // add clone cell to app and instantiate resulting clone cell
        let (_, installed_clone_cell) = self
            .update_state_prime(move |mut state| {
                let app = state.get_app_mut(&app_id)?;
                let agent_key = app.role(&role_name)?.agent_key().to_owned();
                let cell_id = CellId::new(clone_dna_hash, agent_key);
                let clone_id = app.add_clone(&role_name, &cell_id)?;
                let installed_clone_cell =
                    InstalledCell::new(cell_id, clone_id.as_app_role_name().clone());
                Ok((state, installed_clone_cell))
            })
            .await?;

        // register clone cell dna in ribosome store
        self.register_dna(clone_dna).await?;
        Ok(installed_clone_cell)
    }

    /// Print the current setup in a machine readable way
    fn print_setup(&self) {
        use std::fmt::Write;
        let mut out = String::new();
        self.admin_websocket_ports
            .share_ref(|admin_websocket_ports| {
                for port in admin_websocket_ports {
                    writeln!(&mut out, "###ADMIN_PORT:{}###", port)
                        .expect("Can't write setup to std out");
                }
            });
        println!("\n###HOLOCHAIN_SETUP###\n{}###HOLOCHAIN_SETUP_END###", out);
    }
}

/// Methods only available with feature "test_utils"
#[cfg(any(test, feature = "test_utils"))]
#[allow(missing_docs)]
mod test_utils_impls {
    use super::*;

    impl Conductor {
        pub async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
            self.get_state().await
        }

        pub async fn add_test_app_interface<I: Into<AppInterfaceId>>(
            &self,
            id: I,
        ) -> ConductorResult<()> {
            let id = id.into();
            let (signal_tx, _r) = tokio::sync::broadcast::channel(1000);
            self.app_interfaces.share_mut(|app_interfaces| {
                if app_interfaces.contains_key(&id) {
                    return Err(ConductorError::AppInterfaceIdCollision(id));
                }
                let _ = app_interfaces.insert(id, AppInterfaceRuntime::Test { signal_tx });
                Ok(())
            })
        }

        pub fn get_authored_db(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorApiResult<DbWrite<DbKindAuthored>> {
            Ok(self.get_or_create_authored_db(dna_hash)?)
        }

        pub fn get_dht_db(&self, dna_hash: &DnaHash) -> ConductorApiResult<DbWrite<DbKindDht>> {
            Ok(self.get_or_create_dht_db(dna_hash)?)
        }
        pub fn get_dht_db_cache(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorApiResult<holochain_types::db_cache::DhtDbQueryCache> {
            Ok(self.get_or_create_space(dna_hash)?.dht_query_cache)
        }

        pub fn get_cache_db(&self, cell_id: &CellId) -> ConductorApiResult<DbWrite<DbKindCache>> {
            let cell = self.cell_by_id(cell_id)?;
            Ok(cell.cache().clone())
        }

        pub fn get_p2p_db(&self, space: &DnaHash) -> DbWrite<DbKindP2pAgents> {
            self.p2p_agents_db(space)
        }

        pub fn get_p2p_metrics_db(&self, space: &DnaHash) -> DbWrite<DbKindP2pMetrics> {
            self.p2p_metrics_db(space)
        }

        pub fn get_spaces(&self) -> Spaces {
            self.spaces.clone()
        }

        pub fn get_cell_triggers(&self, cell_id: &CellId) -> ConductorApiResult<QueueTriggers> {
            let cell = self.cell_by_id(cell_id)?;
            Ok(cell.triggers().clone())
        }
    }
}

/// Perform Genesis on the source chains for each of the specified CellIds.
///
/// If genesis fails for any cell, this entire function fails, and all other
/// partial or complete successes are rolled back.
/// Note this function takes read locks so should not be called from within a read lock.
pub(crate) async fn genesis_cells(
    conductor: ConductorHandle,
    cell_ids_with_proofs: Vec<(CellId, Option<MembraneProof>)>,
) -> ConductorResult<()> {
    let cells_tasks = cell_ids_with_proofs.into_iter().map(|(cell_id, proof)| {
        let space = conductor
            .get_or_create_space(cell_id.dna_hash())
            .map_err(|e| CellError::FailedToCreateDnaSpace(e.into()));
        async {
            let space = space?;
            let authored_db = space.authored_db;
            let dht_db = space.dht_db;
            let dht_db_cache = space.dht_query_cache;
            let conductor = conductor.clone();
            let chc = conductor.chc(&cell_id);
            let cell_id_inner = cell_id.clone();
            let ribosome = conductor
                .get_ribosome(cell_id.dna_hash())
                .map_err(Box::new)?;
            tokio::spawn(async move {
                Cell::genesis(
                    cell_id_inner,
                    conductor,
                    authored_db,
                    dht_db,
                    dht_db_cache,
                    ribosome,
                    proof,
                    chc,
                )
                .await
            })
            .map_err(CellError::from)
            .and_then(|result| async move { result.map(|_| cell_id) })
            .await
        }
    });
    let (success, errors): (Vec<_>, Vec<_>) = futures::future::join_all(cells_tasks)
        .await
        .into_iter()
        .partition(Result::is_ok);

    // unwrap safe because of the partition
    // TODO: Reference count the databases created here and clean them up on error.
    let _success = success.into_iter().map(Result::unwrap);

    // If there were errors, cleanup and return the errors
    if !errors.is_empty() {
        // match needed to avoid Debug requirement on unwrap_err
        let errors = errors
            .into_iter()
            .map(|e| match e {
                Err(e) => e,
                Ok(_) => unreachable!("Safe because of the partition"),
            })
            .collect();

        Err(ConductorError::GenesisFailed { errors })
    } else {
        // No errors so return the cells
        Ok(())
    }
}

/// Dump the integration json state.
pub async fn integration_dump(
    vault: &DbRead<DbKindDht>,
) -> ConductorApiResult<IntegrationStateDump> {
    vault
        .async_reader(move |txn| {
            let integrated = txn.query_row(
                "SELECT count(hash) FROM DhtOp WHERE when_integrated IS NOT NULL",
                [],
                |row| row.get(0),
            )?;
            let integration_limbo = txn.query_row(
            "SELECT count(hash) FROM DhtOp WHERE when_integrated IS NULL AND validation_stage = 3",
            [],
            |row| row.get(0),
        )?;
            let validation_limbo = txn.query_row(
                "
                SELECT count(hash) FROM DhtOp
                WHERE when_integrated IS NULL
                AND
                (validation_stage IS NULL OR validation_stage < 3)
                ",
                [],
                |row| row.get(0),
            )?;
            ConductorApiResult::Ok(IntegrationStateDump {
                validation_limbo,
                integration_limbo,
                integrated,
            })
        })
        .await
}

/// Dump the full integration json state.
/// Careful! This will return a lot of data.
pub async fn full_integration_dump(
    vault: &DbRead<DbKindDht>,
    dht_ops_cursor: Option<u64>,
) -> ConductorApiResult<FullIntegrationStateDump> {
    vault
        .async_reader(move |txn| {
            let integrated =
                query_dht_ops_from_statement(&txn, state_dump::DHT_OPS_INTEGRATED, dht_ops_cursor)?;

            let validation_limbo = query_dht_ops_from_statement(
                &txn,
                state_dump::DHT_OPS_IN_VALIDATION_LIMBO,
                dht_ops_cursor,
            )?;

            let integration_limbo = query_dht_ops_from_statement(
                &txn,
                state_dump::DHT_OPS_IN_INTEGRATION_LIMBO,
                dht_ops_cursor,
            )?;

            let dht_ops_cursor = txn
                .query_row(state_dump::DHT_OPS_ROW_ID, [], |row| row.get(0))
                .unwrap_or(0);

            ConductorApiResult::Ok(FullIntegrationStateDump {
                validation_limbo,
                integration_limbo,
                integrated,
                dht_ops_cursor,
            })
        })
        .await
}

fn query_dht_ops_from_statement(
    txn: &Transaction,
    stmt_str: &str,
    dht_ops_cursor: Option<u64>,
) -> ConductorApiResult<Vec<DhtOp>> {
    let final_stmt_str = match dht_ops_cursor {
        Some(cursor) => format!("{} AND rowid > {}", stmt_str, cursor),
        None => stmt_str.into(),
    };

    let mut stmt = txn.prepare(final_stmt_str.as_str())?;

    let r: Vec<DhtOp> = stmt
        .query_and_then([], |row| {
            let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
            let op_type: DhtOpType = row.get("dht_type")?;
            let entry = match action.0.entry_type().map(|et| et.visibility()) {
                Some(EntryVisibility::Public) => {
                    let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                    match entry {
                        Some(entry) => Some(from_blob::<Entry>(entry)?),
                        None => None,
                    }
                }
                _ => None,
            };
            Ok(DhtOp::from_type(op_type, action, entry)?)
        })?
        .collect::<StateQueryResult<Vec<_>>>()?;
    Ok(r)
}

// #[instrument(skip(p2p_evt, handle))]
async fn p2p_event_task(
    p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
    handle: ConductorHandle,
) {
    /// The number of events we allow to run in parallel before
    /// starting to await on the join handles.
    const NUM_PARALLEL_EVTS: usize = 100;
    let num_tasks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_time = Arc::new(std::sync::atomic::AtomicU64::new(0));
    p2p_evt
        .for_each_concurrent(NUM_PARALLEL_EVTS, |evt| {
            let handle = handle.clone();
            let num_tasks = num_tasks.clone();
            let max_time = max_time.clone();
            async move {
                let start = (num_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
                    >= NUM_PARALLEL_EVTS)
                    .then(std::time::Instant::now);

                if let Err(e) = handle.dispatch_holochain_p2p_event(evt).await {
                    tracing::error!(
                        message = "error dispatching network event",
                        error = ?e,
                    );
                }
                match start {
                    Some(start) => {
                        let el = start.elapsed();
                        let us = el.as_micros() as u64;
                        let max_us = max_time
                            .fetch_max(us, std::sync::atomic::Ordering::Relaxed)
                            .max(us);

                        let s = tracing::info_span!("holochain_perf", this_event_time = ?el, max_event_micros = %max_us);
                        s.in_scope(|| tracing::info!("dispatch_holochain_p2p_event is saturated"))
                    }
                    None => max_time.store(0, std::sync::atomic::Ordering::Relaxed),
                }
                num_tasks.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            .in_current_span()
        })
        .await;

    tracing::warn!("p2p_event_task has ended");
}

#[cfg(test)]
pub mod tests;
