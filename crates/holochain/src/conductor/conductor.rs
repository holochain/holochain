#![deny(missing_docs)]
#![allow(deprecated)]

//! A Conductor is a dynamically changing group of [Cell]s.
//!
//! A Conductor can be managed:
//! - externally, via an [`AppInterfaceApi`]
//! - from within a [`Cell`], via [`CellConductorApi`](super::api::CellConductorApi)
//!
//! In normal use cases, a single Holochain user runs a single Conductor in a single process.
//! However, there's no reason we can't have multiple Conductors in a single process, simulating multiple
//! users in a testing environment.
//!
//! ```rust, no_run
//! async fn async_main () {
//! use holochain_state::test_utils::test_db_dir;
//! use holochain::conductor::{Conductor, ConductorBuilder};
//! use holochain::conductor::ConductorHandle;
//!
//! let env_dir = test_db_dir();
//! let conductor: ConductorHandle = ConductorBuilder::new()
//!    .test(&[])
//!    .await
//!    .unwrap();
//!
//! // conductors are cloneable
//! let conductor2 = conductor.clone();
//!
//! assert_eq!(conductor.list_dnas(), vec![]);
//! conductor.shutdown();
//!
//! }
//! ```

/// Name of the wasm cache folder within the data root directory.
pub const WASM_CACHE: &str = "wasm-cache";

pub use self::share::RwShare;
use super::api::error::ConductorApiError;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
use holochain_wasmer_host::module::ModuleCache;
use itertools::Itertools;
use rusqlite::Transaction;
use tokio::sync::mpsc::error::SendError;
use tokio::task::JoinHandle;
use tracing::*;

pub use agent_key_operations::RevokeAgentKeyForAppResult;
pub use builder::*;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::{DpkiConfig, KeystoreConfig};
use holochain_conductor_api::AppInfo;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::FullIntegrationStateDump;
use holochain_conductor_api::FullStateDump;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::JsonDump;
pub use holochain_conductor_services::*;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::event::HolochainP2pEvent;
use holochain_p2p::DnaHashExt;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::sql::sql_cell::state_dump;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::nonce::witness_nonce;
use holochain_state::nonce::WitnessNonceResult;
use holochain_state::prelude::*;
use holochain_state::source_chain;
pub use holochain_types::share;
use holochain_zome_types::prelude::{ClonedCell, Signature, Timestamp};
use kitsune_p2p::agent_store::AgentInfoSigned;

use crate::conductor::cell::Cell;
use crate::conductor::conductor::app_auth_token_store::AppAuthTokenStore;
use crate::conductor::conductor::app_broadcast::AppBroadcast;
use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorResult;
use crate::conductor::metrics::create_p2p_event_duration_metric;
use crate::conductor::p2p_agent_store::get_single_agent_info;
use crate::conductor::p2p_agent_store::list_all_agent_info;
use crate::conductor::p2p_agent_store::query_peer_density;
use crate::core::queue_consumer::InitialQueueTriggers;
use crate::core::queue_consumer::QueueConsumerMap;
#[cfg(any(test, feature = "test_utils"))]
use crate::core::queue_consumer::QueueTriggers;
use crate::core::ribosome::guest_callback::post_commit::PostCommitArgs;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CHANNEL_BOUND;
use crate::core::ribosome::guest_callback::post_commit::POST_COMMIT_CONCURRENT_LIMIT;
use crate::core::ribosome::real_ribosome::ModuleCacheLock;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::ZomeCallResult;
use crate::{
    conductor::api::error::ConductorApiResult, core::ribosome::real_ribosome::RealRibosome,
};

use super::api::AppInterfaceApi;
use super::api::ZomeCall;
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
use super::entry_def_store::get_entry_defs;
use super::error::ConductorError;
use super::interface::error::InterfaceResult;
use super::interface::websocket::spawn_admin_interface_tasks;
use super::interface::websocket::spawn_app_interface_task;
use super::interface::websocket::spawn_websocket_listener;
use super::manager::TaskManagerResult;
use super::p2p_agent_store;
use super::p2p_agent_store::P2pBatch;
use super::p2p_agent_store::*;
use super::ribosome_store::RibosomeStore;
use super::space::Space;
use super::space::Spaces;
use super::state::AppInterfaceConfig;
use super::state::AppInterfaceId;
use super::state::ConductorState;
use super::CellError;
use super::{api::AdminInterfaceApi, manager::TaskManagerClient};

mod builder;

mod chc;

mod graft_records_onto_source_chain;

mod app_auth_token_store;

/// Operations to manipulate agent keys.
///
/// Agent keys are handled in 2 places in Holochain, on the source chain of a cell and in the
/// Deepkey service, should it be installed. Operations to manipulate these keys include key
/// revocation and key update.
///
/// When revoking a key, it becomes invalid and the source chain can no longer be written to.
/// Clone cells can not be created any more either. This source chain state if final and can not
/// be reverted or changed.
mod agent_key_operations;

pub(crate) mod app_broadcast;

#[cfg(test)]
pub mod tests;

/// How long we should attempt to achieve a "network join" when first activating a cell,
/// before moving on and letting the network health activity go on in the background.
///
/// This gives us a chance to start an app in an "online" state, increasing the probability
/// of an app having full network access as soon as its UI begins making requests.
pub const JOIN_NETWORK_WAITING_PERIOD: std::time::Duration = std::time::Duration::from_secs(5);

/// A list of Cells which failed to start, and why
pub type CellStartupErrors = Vec<(CellId, CellError)>;

/// Cloneable reference to a Conductor
pub type ConductorHandle = Arc<Conductor>;

/// Legacy CellStatus which is no longer used. This can be removed
/// and is only here to avoid breaking deserialization specs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[deprecated = "Only here for deserialization, should be removed altogether when all clients are updated"]
pub enum CellStatus {
    /// Kitsune knows about this Cell and it is considered fully "online"
    Joined,

    /// The Cell is on its way to being fully joined. It is a valid Cell from
    /// the perspective of the conductor, and can handle HolochainP2pEvents,
    /// but it is considered not to be fully running from the perspective of
    /// app status, i.e. if any app has a required Cell with this status,
    /// the app is considered to be in the Paused state.
    PendingJoin(PendingJoinReason),

    /// The Cell is currently in the process of trying to join the network.
    Joining,
}

/// The reason why a cell is waiting to join the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingJoinReason {
    /// The initial state, no attempt has been made to join the network yet.
    Initial,

    /// The join failed with an error that is safe to retry, such as not
    /// being connected to the internet.
    Retry(String),

    /// The network join failed and will not be retried. This will impact
    /// the status of the associated
    /// app and require manual intervention from the user.
    Failed(String),

    /// The join attempt has timed out.
    TimedOut,
}

/// A [`Cell`] tracked by a Conductor, along with its [`CellStatus`]
#[derive(Debug, Clone)]
#[allow(deprecated)]
#[allow(unused)]
struct CellItem {
    cell: Arc<Cell>,
    status: CellStatus,
}

#[allow(dead_code)]
pub(crate) type StopBroadcaster = task_motel::StopBroadcaster;
pub(crate) type StopReceiver = task_motel::StopListener;

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    /// The collection of available, running cells associated with this Conductor
    running_cells: RwShare<HashMap<CellId, CellItem>>,

    /// The config used to create this Conductor
    pub config: Arc<ConductorConfig>,

    /// The map of dna hash spaces.
    pub(crate) spaces: Spaces,

    /// Set to true when `conductor.shutdown()` has been called, so that other
    /// tasks can check on the shutdown status
    shutting_down: Arc<AtomicBool>,

    /// The admin websocket ports this conductor has open.
    /// This exists so that we can run tests and bind to port 0, and find out
    /// the dynamically allocated port later.
    admin_websocket_ports: RwShare<Vec<u16>>,

    /// The interface to the task manager
    task_manager: TaskManagerClient,

    /// The JoinHandle for the long-running task which processes the outcomes of ended tasks,
    /// taking actions like disabling cells or shutting down the conductor on errors.
    /// It terminates only when the TaskManager and all of its tasks have ended and dropped.
    pub(crate) outcomes_task: RwShare<Option<JoinHandle<TaskManagerResult>>>,

    /// Placeholder for what will be the real DNA/Wasm cache
    ribosome_store: RwShare<RibosomeStore>,

    /// Access to private keys for signing and encryption.
    keystore: MetaLairClient,

    /// Handle to the network actor.
    holochain_p2p: holochain_p2p::HolochainP2pRef,

    post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,

    scheduler: Arc<parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    pub(crate) running_services: RwShare<ConductorServices>,

    /// File system and in-memory cache for wasmer modules.
    // Used in ribosomes but kept here as a single instance.
    pub(crate) wasmer_module_cache: Arc<ModuleCacheLock>,

    app_auth_token_store: RwShare<AppAuthTokenStore>,

    /// Container to connect app signals to app interfaces, by installed app id.
    app_broadcast: AppBroadcast,
}

impl Conductor {
    /// Create a conductor builder.
    pub fn builder() -> ConductorBuilder {
        ConductorBuilder::new()
    }
}

/// Methods related to conductor startup/shutdown
mod startup_shutdown_impls {

    use crate::conductor::manager::{spawn_task_outcome_handler, OutcomeReceiver, OutcomeSender};

    use super::*;

    //-----------------------------------------------------------------------------
    /// Methods used by the [ConductorHandle]
    //-----------------------------------------------------------------------------
    impl Conductor {
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn new(
            config: Arc<ConductorConfig>,
            ribosome_store: RwShare<RibosomeStore>,
            keystore: MetaLairClient,
            holochain_p2p: holochain_p2p::HolochainP2pRef,
            spaces: Spaces,
            post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,
            outcome_sender: OutcomeSender,
        ) -> Self {
            let tracing_scope = config.tracing_scope().unwrap_or_default();
            let maybe_data_root_path = config.data_root_path.clone().map(|path| (*path).clone());

            if let Some(path) = &maybe_data_root_path {
                let mut path = path.clone();
                path.push(WASM_CACHE);

                // best effort to ensure the cache dir exists if configured
                let _ = std::fs::create_dir_all(&path);
            }

            Self {
                spaces,
                running_cells: RwShare::new(HashMap::new()),
                config,
                shutting_down: Arc::new(AtomicBool::new(false)),
                task_manager: TaskManagerClient::new(outcome_sender, tracing_scope),
                // Must be initialized later, since it requires an Arc<Conductor>
                outcomes_task: RwShare::new(None),
                admin_websocket_ports: RwShare::new(Vec::new()),
                scheduler: Arc::new(parking_lot::Mutex::new(None)),
                ribosome_store,
                keystore,
                holochain_p2p,
                post_commit,
                running_services: RwShare::new(ConductorServices::default()),
                wasmer_module_cache: Arc::new(ModuleCacheLock::new(ModuleCache::new(
                    maybe_data_root_path.map(|p| p.join(WASM_CACHE)),
                ))),
                app_auth_token_store: RwShare::default(),
                app_broadcast: AppBroadcast::default(),
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

        /// Take ownership of the TaskManagerClient as well as the task which completes
        /// when all managed tasks have completed
        pub fn detach_task_management(&self) -> Option<JoinHandle<TaskManagerResult>> {
            self.outcomes_task.share_mut(|tm| tm.take())
        }

        /// Broadcasts the shutdown signal to all managed tasks
        /// and returns a future to await for shutdown to complete.
        pub fn shutdown(&self) -> JoinHandle<TaskManagerResult> {
            self.shutting_down
                .store(true, std::sync::atomic::Ordering::Relaxed);

            use ghost_actor::GhostControlSender;
            let ghost_shutdown = self.holochain_p2p.ghost_actor_shutdown_immediate();
            let mut tm = self.task_manager();
            let task = self.detach_task_management().expect("Attempting to shut down after already detaching task management or previous shutdown");
            tokio::task::spawn(async move {
                tracing::info!("Sending shutdown signal to all managed tasks.");
                let (_, _, r) = futures::join!(ghost_shutdown, tm.shutdown().boxed(), task,);
                r?
            })
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(scope=self.config.network.tracing_scope)))]
        pub(crate) async fn initialize_conductor(
            self: Arc<Self>,
            outcome_rx: OutcomeReceiver,
            admin_configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<CellStartupErrors> {
            self.load_dnas().await?;

            info!("Conductor startup: DNAs loaded.");

            // Start the task manager
            self.outcomes_task.share_mut(|lock| {
                if lock.is_some() {
                    panic!("Cannot start task manager twice");
                }
                let task = spawn_task_outcome_handler(self.clone(), outcome_rx);
                *lock = Some(task);
            });

            self.clone().initialize_services().await?;
            self.clone().add_admin_interfaces(admin_configs).await?;

            info!("Conductor startup: admin interface(s) added.");

            self.clone().startup_app_interfaces().await?;

            info!("Conductor startup: app interfaces started.");

            // We don't care what fx are returned here, since all cells need to
            // be spun up
            let _ = self.start_paused_apps().await?;
            let res = self.process_app_status_fx(AppStatusFx::SpinUp, None).await;

            info!("Conductor startup: apps started.");

            res
        }
    }
}

/// Methods related to conductor interfaces
mod interface_impls {
    use super::*;
    use holochain_conductor_api::AppInterfaceInfo;
    use holochain_types::websocket::AllowedOrigins;

    impl Conductor {
        /// Spawn all admin interface tasks, register them with the TaskManager,
        /// and modify the conductor accordingly, based on the config passed in
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn add_admin_interfaces(
            self: Arc<Self>,
            configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<Vec<u16>> {
            let admin_api = AdminInterfaceApi::new(self.clone());
            let tm = self.task_manager();

            // Closure to process each admin config item
            let spawn_from_config = |AdminInterfaceConfig { driver, .. }| {
                let admin_api = admin_api.clone();
                let tm = tm.clone();
                async move {
                    match driver {
                        InterfaceDriver::Websocket {
                            port,
                            allowed_origins,
                        } => {
                            let listener = spawn_websocket_listener(port, allowed_origins).await?;
                            let port = listener.local_addrs()?[0].port();
                            spawn_admin_interface_tasks(
                                tm.clone(),
                                listener,
                                admin_api.clone(),
                                port,
                            );

                            InterfaceResult::Ok(port)
                        }
                    }
                }
            };

            // spawn interface tasks, collect their JoinHandles,
            // panic on errors.
            let ports: Result<Vec<_>, _> =
                future::join_all(configs.into_iter().map(spawn_from_config))
                    .await
                    .into_iter()
                    .collect();
            // Exit if the admin interfaces fail to be created
            let ports = ports.map_err(Box::new)?;

            for p in &ports {
                self.add_admin_port(*p);
            }

            Ok(ports)
        }

        /// Spawn a new app interface task, register it with the TaskManager,
        /// and modify the conductor accordingly, based on the config passed in.
        ///
        /// Returns the given or auto-chosen port number if giving an Ok Result
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn add_app_interface(
            self: Arc<Self>,
            port: either::Either<u16, AppInterfaceId>,
            allowed_origins: AllowedOrigins,
            installed_app_id: Option<InstalledAppId>,
        ) -> ConductorResult<u16> {
            let interface_id = match port {
                either::Either::Left(port) => AppInterfaceId::new(port),
                either::Either::Right(id) => id,
            };
            let port = interface_id.port();
            debug!("Attaching interface {}", port);
            let app_api = AppInterfaceApi::new(self.clone());

            let tm = self.task_manager();

            // TODO: RELIABILITY: Handle this task by restarting it if it fails and log the error
            let port = spawn_app_interface_task(
                tm.clone(),
                port,
                allowed_origins.clone(),
                installed_app_id.clone(),
                app_api,
                self.app_broadcast.clone(),
            )
            .await
            .map_err(Box::new)?;

            let config = AppInterfaceConfig::websocket(port, allowed_origins, installed_app_id);
            self.update_state(|mut state| {
                state.app_interfaces.insert(interface_id, config);

                Ok(state)
            })
            .await?;
            debug!("App interface added at port: {}", port);
            Ok(port)
        }

        /// Returns a port which is guaranteed to have a websocket listener with an Admin interface
        /// on it. Useful for specifying port 0 and letting the OS choose a free port.
        pub fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
            self.admin_websocket_ports.share_ref(|p| p.first().copied())
        }

        /// Give a list of networking ports taken up as running app interface tasks
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_app_interfaces(&self) -> ConductorResult<Vec<AppInterfaceInfo>> {
            Ok(self
                .get_state()
                .await?
                .app_interfaces
                .values()
                .map(|config| AppInterfaceInfo {
                    port: config.driver.port(),
                    allowed_origins: config.driver.allowed_origins().clone(),
                    installed_app_id: config.installed_app_id.clone(),
                })
                .collect())
        }

        /// Start all app interfaces currently in state.
        /// This should only be run at conductor initialization.
        #[allow(irrefutable_let_patterns)]
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn startup_app_interfaces(self: Arc<Self>) -> ConductorResult<()> {
            for (id, config) in &self.get_state().await?.app_interfaces {
                debug!("Starting up app interface: {:?}", id);
                let _ = self
                    .clone()
                    .add_app_interface(
                        either::Right(id.clone()),
                        config.driver.allowed_origins().clone(),
                        config.installed_app_id.clone(),
                    )
                    .await?;
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
            let dpki_dna_hash = self
                .running_services()
                .dpki
                .map(|dpki| dpki.cell_id.dna_hash().clone());
            let mut hashes = self.ribosome_store().share_ref(|ds| ds.list());
            if let Some(dpki_dna_hash) = dpki_dna_hash {
                hashes.retain(|h| *h != dpki_dna_hash);
            }
            hashes
        }

        /// Get a [`DnaDef`] from the [`RibosomeStore`]
        pub fn get_dna_def(&self, hash: &DnaHash) -> Option<DnaDef> {
            self.ribosome_store().share_ref(|ds| ds.get_dna_def(hash))
        }

        /// Get a [`DnaFile`] from the [`RibosomeStore`]
        pub fn get_dna_file(&self, hash: &DnaHash) -> Option<DnaFile> {
            self.ribosome_store().share_ref(|ds| ds.get_dna_file(hash))
        }

        /// Get an [`EntryDef`] from the [`EntryDefBufferKey`]
        pub fn get_entry_def(&self, key: &EntryDefBufferKey) -> Option<EntryDef> {
            self.ribosome_store().share_ref(|ds| ds.get_entry_def(key))
        }

        /// Create a hash map of all existing DNA definitions, mapped to cell
        /// ids.
        pub fn get_dna_definitions(
            &self,
            app: &InstalledApp,
        ) -> ConductorResult<HashMap<CellId, DnaDefHashed>> {
            let mut dna_defs = HashMap::new();
            for cell_id in app.all_cells() {
                let ribosome = self.get_ribosome(cell_id.dna_hash())?;
                let dna_def = ribosome.dna_def();
                dna_defs.insert(cell_id.to_owned(), dna_def.to_owned());
            }
            Ok(dna_defs)
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
            let (wasms, defs) = db
                .read_async(move |txn| {
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
                    let wasms = holochain_state::dna_def::get_all(&txn)?
                        .into_iter()
                        .map(|dna_def| {
                            // Load all wasms for each dna_def from the wasm db into memory
                            let wasms = dna_def.all_zomes().filter_map(|(zome_name, zome)| {
                                let wasm_hash = zome.wasm_hash(zome_name).ok()?;
                                // Note this is a cheap arc clone.
                                wasms.get(&wasm_hash).cloned()
                            });
                            let wasms = wasms.collect::<Vec<_>>();
                            (dna_def, wasms)
                        })
                        // This needs to happen due to the environment not being Send
                        .collect::<Vec<_>>();
                    let defs = holochain_state::entry_def::get_all(&txn)?;
                    ConductorResult::Ok((wasms, defs))
                })
                .await?;
            // try to join all the tasks and return the list of dna files
            let wasms = wasms.into_iter().map(|(dna_def, wasms)| async move {
                let dna_file = DnaFile::new(dna_def.into_content(), wasms).await;
                let ribosome =
                    RealRibosome::new(dna_file, self.wasmer_module_cache.clone()).await?;
                ConductorResult::Ok((ribosome.dna_hash().clone(), ribosome))
            });
            let dnas = futures::future::try_join_all(wasms).await?;
            Ok((dnas, defs))
        }

        /// Get the root environment directory.
        pub fn root_db_dir(&self) -> &PathBuf {
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
            let to_cleanup: Vec<_> = self.running_cells.share_mut(|cells| {
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
            let code = ribosome.dna_file().code().clone().into_values();
            let zome_defs = get_entry_defs(ribosome).await?;
            self.put_wasm_code(dna_def, code, zome_defs).await
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
                .write_async({
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

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn load_dnas(&self) -> ConductorResult<()> {
            let (ribosomes, entry_defs) = self.load_wasms_into_dna_files().await?;
            self.ribosome_store().share_mut(|ds| {
                ds.add_ribosomes(ribosomes);
                ds.add_entry_defs(entry_defs);
            });
            Ok(())
        }

        /// Install a [`DnaFile`] in this Conductor
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn register_dna(&self, dna: DnaFile) -> ConductorResult<()> {
            if self.get_ribosome(dna.dna_hash()).is_ok() {
                // ribosome for dna is already registered in store
                return Ok(());
            }

            let ribosome = RealRibosome::new(dna, self.wasmer_module_cache.clone()).await?;

            let entry_defs = self.register_dna_wasm(ribosome.clone()).await?;

            self.register_dna_entry_defs(entry_defs);

            self.add_ribosome_to_store(ribosome);

            Ok(())
        }
    }
}

/// Network-related methods
mod network_impls {
    use std::time::Duration;

    use futures::future::join_all;
    use rusqlite::params;

    use holochain_conductor_api::{
        CellInfo, DnaStorageInfo, NetworkInfo, StorageBlob, StorageInfo,
    };
    use holochain_p2p::HolochainP2pSender;
    use holochain_sqlite::stats::{get_size_on_disk, get_used_size};
    use holochain_zome_types::block::Block;
    use holochain_zome_types::block::BlockTargetId;
    use kitsune_p2p::KitsuneAgent;
    use kitsune_p2p::KitsuneBinType;

    use crate::conductor::api::error::{
        zome_call_response_to_conductor_api_result, ConductorApiError,
    };

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

        /// Block some target.
        pub async fn block(&self, input: Block) -> DatabaseResult<()> {
            self.spaces.block(input).await
        }

        /// Unblock some target.
        pub async fn unblock(&self, input: Block) -> DatabaseResult<()> {
            self.spaces.unblock(input).await
        }

        /// Check if some target is blocked.
        pub async fn is_blocked(
            &self,
            input: BlockTargetId,
            timestamp: Timestamp,
        ) -> DatabaseResult<bool> {
            self.spaces.is_blocked(input, timestamp).await
        }

        pub(crate) async fn prune_p2p_agents_db(&self) -> ConductorResult<()> {
            use holochain_p2p::AgentPubKeyExt;

            let mut space_to_agents = HashMap::new();

            for cell in self.running_cells.share_ref(|c| {
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

        pub(crate) async fn network_info(
            &self,
            installed_app_id: &InstalledAppId,
            payload: &NetworkInfoRequestPayload,
        ) -> ConductorResult<Vec<NetworkInfo>> {
            use holochain_sqlite::sql::sql_cell::SUM_OF_RECEIVED_BYTES_SINCE_TIMESTAMP;

            let NetworkInfoRequestPayload {
                agent_pub_key,
                dnas,
                last_time_queried,
            } = payload;

            let app_info = self
                .get_app_info(installed_app_id)
                .await?
                .ok_or_else(|| ConductorError::AppNotInstalled(installed_app_id.clone()))?;

            if agent_pub_key != &app_info.agent_pub_key
                && !app_info
                    .cell_info
                    .values()
                    .flatten()
                    .any(|cell_info| match cell_info {
                        CellInfo::Provisioned(cell) => cell.cell_id.agent_pubkey() == agent_pub_key,
                        _ => false,
                    })
            {
                return Err(ConductorError::AppAccessError(
                    installed_app_id.clone(),
                    Box::new(agent_pub_key.clone()),
                ));
            }

            futures::future::join_all(dnas.iter().map(|dna| async move {
                let diagnostics = self.holochain_p2p.get_diagnostics(dna.clone()).await?;
                let fetch_pool_info = diagnostics
                    .fetch_pool
                    .info([dna.to_kitsune()].into_iter().collect());

                // query number of agents from peer db
                let db = { self.p2p_agents_db(dna) };

                let (current_number_of_peers, arc_size, total_network_peers) = db
                    .read_async({
                        let agent_pub_key = agent_pub_key.clone();
                        let space = dna.clone().into_kitsune();
                        move |txn| -> DatabaseResult<(u32, f64, u32)> {
                            let current_number_of_peers = txn.p2p_count_agents(space.clone())?;

                            // query arc size and extrapolated coverage and estimate total peers
                            let (arc_size, total_network_peers) = match txn.p2p_get_agent(
                                space.clone(),
                                &KitsuneAgent::new(agent_pub_key.get_raw_36().to_vec()),
                            )? {
                                None => (0.0, 0),
                                Some(agent) => {
                                    let arc_size = agent.storage_arc().coverage();
                                    let agents_in_arc = txn.p2p_gossip_query_agents(
                                        space.clone(),
                                        u64::MIN,
                                        u64::MAX,
                                        agent.storage_arc().inner().into(),
                                    )?;
                                    let number_of_agents_in_arc = agents_in_arc.len();
                                    let total_network_peers = if number_of_agents_in_arc == 0 {
                                        0
                                    } else {
                                        (number_of_agents_in_arc as f64 / arc_size) as u32
                                    };
                                    (arc_size, total_network_peers)
                                }
                            };

                            Ok((current_number_of_peers, arc_size, total_network_peers))
                        }
                    })
                    .await?;

                // get sum of bytes from dht and cache db since last time
                // request was made or since the beginning of time
                let last_time_queried = match last_time_queried {
                    Some(timestamp) => *timestamp,
                    None => Timestamp::ZERO,
                };
                let sum_of_bytes_row_fn = |row: &Row| {
                    row.get(0)
                        .map(|maybe_bytes_received: Option<u64>| maybe_bytes_received.unwrap_or(0))
                        .map_err(DatabaseError::SqliteError)
                };
                let dht_db = self
                    .get_or_create_dht_db(dna)
                    .map_err(|err| ConductorError::Other(Box::new(err)))?;
                let dht_bytes_received = dht_db
                    .read_async({
                        move |txn| {
                            txn.query_row_and_then(
                                SUM_OF_RECEIVED_BYTES_SINCE_TIMESTAMP,
                                params![last_time_queried.as_micros()],
                                sum_of_bytes_row_fn,
                            )
                        }
                    })
                    .await?;

                let cache_db = self
                    .get_or_create_cache_db(dna)
                    .map_err(|err| ConductorError::Other(Box::new(err)))?;
                let cache_bytes_received = cache_db
                    .read_async(move |txn| {
                        txn.query_row_and_then(
                            SUM_OF_RECEIVED_BYTES_SINCE_TIMESTAMP,
                            params![last_time_queried.as_micros()],
                            sum_of_bytes_row_fn,
                        )
                    })
                    .await?;
                let bytes_since_last_time_queried = dht_bytes_received + cache_bytes_received;

                // calculate open peer connections based on current gossip sessions
                let completed_rounds_since_last_time_queried = diagnostics
                    .metrics
                    .read()
                    .peer_node_histories()
                    .iter()
                    .flat_map(|(_, node_history)| node_history.completed_rounds.clone())
                    .filter(|completed_round| {
                        let now = tokio::time::Instant::now();
                        let round_start_time_diff = now - completed_round.start_time;
                        let round_start_timestamp =
                            Timestamp::from_micros(round_start_time_diff.as_micros() as i64);
                        round_start_timestamp > last_time_queried
                    })
                    .count() as u32;

                ConductorResult::Ok(NetworkInfo {
                    fetch_pool_info,
                    current_number_of_peers,
                    arc_size,
                    total_network_peers,
                    bytes_since_last_time_queried,
                    completed_rounds_since_last_time_queried,
                })
            }))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn storage_info(&self) -> ConductorResult<StorageInfo> {
            let state = self.get_state().await?;

            let all_dna: HashMap<DnaHash, Vec<InstalledAppId>> = HashMap::new();
            let all_dna = state.installed_apps_and_services().iter().fold(
                all_dna,
                |mut acc, (installed_app_id, app)| {
                    for cell_id in app.all_cells() {
                        acc.entry(cell_id.dna_hash().clone())
                            .or_default()
                            .push(installed_app_id.clone());
                    }

                    acc
                },
            );

            let app_data_blobs =
                futures::future::join_all(all_dna.iter().map(|(dna_hash, used_by)| async {
                    self.storage_info_for_dna(dna_hash, used_by).await
                }))
                .await
                .into_iter()
                .collect::<Result<Vec<StorageBlob>, ConductorError>>()?;

            Ok(StorageInfo {
                blobs: app_data_blobs,
            })
        }

        async fn storage_info_for_dna(
            &self,
            dna_hash: &DnaHash,
            used_by: &Vec<InstalledAppId>,
        ) -> ConductorResult<StorageBlob> {
            let authored_dbs = self.spaces.get_all_authored_dbs(dna_hash)?;
            let dht_db = self.spaces.dht_db(dna_hash)?;
            let cache_db = self.spaces.cache(dna_hash)?;

            Ok(StorageBlob::Dna(DnaStorageInfo {
                authored_data_size_on_disk: join_all(
                    authored_dbs
                        .iter()
                        .map(|db| db.read_async(get_size_on_disk)),
                )
                .await
                .into_iter()
                .map(|r| r.map_err(ConductorError::DatabaseError))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum(),
                authored_data_size: join_all(
                    authored_dbs.iter().map(|db| db.read_async(get_used_size)),
                )
                .await
                .into_iter()
                .map(|r| r.map_err(ConductorError::DatabaseError))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum(),
                dht_data_size_on_disk: dht_db
                    .read_async(get_size_on_disk)
                    .map_err(ConductorError::DatabaseError)
                    .await?,
                dht_data_size: dht_db
                    .read_async(get_used_size)
                    .map_err(ConductorError::DatabaseError)
                    .await?,
                cache_data_size_on_disk: cache_db
                    .read_async(get_size_on_disk)
                    .map_err(ConductorError::DatabaseError)
                    .await?,
                cache_data_size: cache_db
                    .read_async(get_used_size)
                    .map_err(ConductorError::DatabaseError)
                    .await?,
                used_by: used_by.clone(),
            }))
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
                        .send_timeout(
                            P2pBatch {
                                peer_data,
                                result_sender,
                            },
                            Duration::from_secs(10),
                        )
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
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let res = db
                        .p2p_gossip_query_agents(since_ms, until_ms, (*arc_set).clone())
                        .await
                        .map_err(holochain_p2p::HolochainP2pError::other);

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
                        .tuning_params
                        .danger_gossip_recent_threshold();
                    let topo = self
                        .get_dna_def(&dna_hash)
                        .ok_or_else(|| DnaError::DnaMissing(dna_hash.clone()))?
                        .topology(cutoff);
                    let tuning = self.get_config().kitsune_tuning_params();
                    let db = { self.p2p_agents_db(&dna_hash) };
                    let res = query_peer_density(
                        db.into(),
                        topo,
                        tuning.to_arq_strat().into(),
                        kitsune_space,
                        dht_arc,
                    )
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
                | CountLinks { .. }
                | GetAgentActivity { .. }
                | MustGetAgentActivity { .. }
                | ValidationReceiptsReceived { .. } => {
                    let cell_id =
                        CellId::new(event.dna_hash().clone(), event.target_agents().clone());
                    let cell = self.cell_by_id(&cell_id).await?;
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

        /// List all host functions provided by this conductor for wasms.
        pub async fn list_wasm_host_functions(&self) -> ConductorApiResult<Vec<String>> {
            Ok(RealRibosome::tooling_imports().await?)
        }

        /// Invoke a zome function on a Cell
        pub async fn call_zome(&self, call: ZomeCall) -> ConductorApiResult<ZomeCallResult> {
            let cell = self.cell_by_id(&call.cell_id).await?;
            Ok(cell.call_zome(call, None).await?)
        }

        pub(crate) async fn call_zome_with_workspace(
            &self,
            call: ZomeCall,
            workspace_lock: SourceChainWorkspace,
        ) -> ConductorApiResult<ZomeCallResult> {
            debug!(cell_id = ?call.cell_id);
            let cell = self.cell_by_id(&call.cell_id).await?;
            Ok(cell.call_zome(call, Some(workspace_lock)).await?)
        }

        /// Make a zome call with deserialization and some error unwrapping built in
        pub async fn easy_call_zome<I, O, Z>(
            &self,
            provenance: &AgentPubKey,
            cap_secret: Option<CapSecret>,
            cell_id: CellId,
            zome_name: Z,
            fn_name: impl Into<FunctionName>,
            payload: I,
        ) -> ConductorApiResult<O>
        where
            ZomeName: From<Z>,
            I: Serialize + std::fmt::Debug,
            O: serde::de::DeserializeOwned + std::fmt::Debug,
        {
            let payload = ExternIO::encode(payload).expect("Couldn't serialize payload");
            let now = Timestamp::now();
            let (nonce, expires_at) =
                holochain_nonce::fresh_nonce(now).map_err(ConductorApiError::Other)?;
            let call_unsigned = ZomeCallUnsigned {
                cell_id,
                zome_name: zome_name.into(),
                fn_name: fn_name.into(),
                cap_secret,
                provenance: provenance.clone(),
                payload,
                nonce,
                expires_at,
            };
            let call =
                ZomeCall::try_from_unsigned_zome_call(self.keystore(), call_unsigned).await?;
            let response = self.call_zome(call).await;
            match response {
                Ok(Ok(response)) => Ok(zome_call_response_to_conductor_api_result(response)?),
                Ok(Err(error)) => Err(ConductorApiError::Other(Box::new(error))),
                Err(error) => Err(error),
            }
        }
    }
}

/// Methods related to app installation and management
mod app_impls {
    use holochain_types::deepkey_roundtrip_backward;

    use crate::conductor::state::is_app;

    use super::*;

    impl Conductor {
        /// Install an app from minimal elements, without needing construct a whole AppBundle.
        /// (This function constructs a bundle under the hood.)
        /// This is just a convenience for testing.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn install_app_minimal(
            self: Arc<Self>,
            installed_app_id: InstalledAppId,
            agent: Option<AgentPubKey>,
            data: &[(impl DnaWithRole, Option<MembraneProof>)],
        ) -> ConductorResult<AgentPubKey> {
            let dnas_with_roles: Vec<_> = data.iter().map(|(dr, _)| dr).cloned().collect();
            let manifest = app_manifest_from_dnas(&dnas_with_roles, 255, false);

            let (dnas_to_register, role_assignments): (Vec<_>, Vec<_>) = data
                .iter()
                .map(|(dr, mp)| {
                    let dna = dr.dna().clone();
                    let dna_hash = dna.dna_hash().clone();
                    let dnas_to_register = (dna, mp.clone());
                    let role_assignments =
                        (dr.role(), AppRolePrimary::new(dna_hash, true, 255).into());
                    (dnas_to_register, role_assignments)
                })
                .unzip();

            let ops = AppRoleResolution {
                dnas_to_register,
                role_assignments,
            };

            let app = self
                .install_app_common(installed_app_id, manifest, agent.clone(), false, ops, false)
                .await?;

            Ok(app.agent_key().clone())
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        async fn install_app_common(
            self: Arc<Self>,
            installed_app_id: InstalledAppId,
            manifest: AppManifest,
            agent_key: Option<AgentPubKey>,
            defer_memproofs: bool,
            ops: AppRoleResolution,
            ignore_genesis_failure: bool,
        ) -> ConductorResult<InstalledApp> {
            let dpki = self.running_services().dpki.clone();

            // if dpki is installed, load dpki state
            let mut dpki = if let Some(d) = dpki.as_ref() {
                let lock = d.state().await;
                Some((d.clone(), lock))
            } else {
                None
            };

            let (agent_key, derivation_details): (AgentPubKey, Option<DerivationDetailsInput>) =
                if let Some(agent_key) = agent_key {
                    if dpki.is_some() {
                        // dpki installed, agent key given
                        tracing::warn!("App is being installed with an existing agent key: DPKI will not be used to manage keys for this app.");
                    }
                    (agent_key, None)
                } else if let Some((dpki, state)) = &mut dpki {
                    // dpki installed, no agent key given

                    // register a new key derivation for this app
                    let derivation_details = state.next_derivation_details(None).await?;

                    let dst_tag = format!(
                        "DPKI-{:04}-{:04}",
                        derivation_details.app_index, derivation_details.key_index
                    );

                    let derivation_path = derivation_details.to_derivation_path();
                    let derivation_bytes = derivation_path
                        .iter()
                        .flat_map(|c| c.to_be_bytes())
                        .collect();

                    let info = self
                        .keystore
                        .lair_client()
                        .derive_seed(
                            dpki.device_seed_lair_tag.clone().into(),
                            None,
                            dst_tag.into(),
                            None,
                            derivation_path.into_boxed_slice(),
                        )
                        .await
                        .map_err(|e| DpkiServiceError::Lair(e.into()))?;
                    let seed = info.ed25519_pub_key.0.to_vec();

                    let derivation = DerivationDetailsInput {
                        app_index: derivation_details.app_index,
                        key_index: derivation_details.key_index,
                        derivation_seed: seed.clone(),
                        derivation_bytes,
                    };

                    (AgentPubKey::from_raw_32(seed), Some(derivation))
                } else {
                    // dpki not installed, no agent key given
                    (self.keystore.new_sign_keypair_random().await?, None)
                };

            let cells_to_create = ops.cells_to_create(agent_key.clone());

            // check if cells_to_create contains a cell identical to an existing one
            let state = self.get_state().await?;
            let all_cells: HashSet<_> = state
                .installed_apps_and_services()
                .values()
                .flat_map(|app| app.all_cells())
                .collect();
            let maybe_duplicate_cell_id = cells_to_create
                .iter()
                .find(|(cell_id, _)| all_cells.contains(cell_id));
            if let Some((duplicate_cell_id, _)) = maybe_duplicate_cell_id {
                return Err(ConductorError::CellAlreadyExists(
                    duplicate_cell_id.to_owned(),
                ));
            };

            for (dna, _) in ops.dnas_to_register {
                self.clone().register_dna(dna).await?;
            }

            let cell_ids: Vec<_> = cells_to_create
                .iter()
                .map(|(cell_id, _)| cell_id.clone())
                .collect();

            let app_result = if defer_memproofs {
                let roles = ops.role_assignments;
                let app = InstalledAppCommon::new(
                    installed_app_id.clone(),
                    agent_key.clone(),
                    roles,
                    manifest,
                )?;

                let (_, app) = self
                    .update_state_prime(move |mut state| {
                        let app = state.add_app_awaiting_memproofs(app)?;
                        Ok((state, app))
                    })
                    .await?;
                Ok(app)
            } else {
                let genesis_result =
                    crate::conductor::conductor::genesis_cells(self.clone(), cells_to_create).await;

                if genesis_result.is_ok() || ignore_genesis_failure {
                    let roles = ops.role_assignments;
                    let app = InstalledAppCommon::new(
                        installed_app_id.clone(),
                        agent_key.clone(),
                        roles,
                        manifest,
                    )?;

                    // Update the db
                    let stopped_app = self.add_disabled_app_to_db(app).await?;

                    // Return the result, which be may be an error if no_rollback was specified
                    genesis_result.map(|()| stopped_app.into())
                } else if let Err(err) = genesis_result {
                    // Rollback created cells on error
                    self.remove_cells(&cell_ids).await;
                    Err(err)
                } else {
                    unreachable!()
                }
            };

            if app_result.is_ok() {
                // Register the key in DPKI

                // Register initial agent key in Deepkey
                if let (Some((dpki, state)), Some(derivation)) = (dpki, derivation_details) {
                    let dpki_agent = dpki.cell_id.agent_pubkey();

                    // This is the signature Deepkey requires
                    let signature = agent_key
                        .sign_raw(&self.keystore, dpki_agent.get_raw_39().into())
                        .await
                        .map_err(|e| DpkiServiceError::Lair(e.into()))?;

                    let signature = deepkey_roundtrip_backward!(Signature, &signature);

                    let dna_hashes = cell_ids
                        .iter()
                        .map(|c| deepkey_roundtrip_backward!(DnaHash, c.dna_hash()))
                        .collect();

                    let agent_key = deepkey_roundtrip_backward!(AgentPubKey, &agent_key);

                    let input = CreateKeyInput {
                        key_generation: KeyGeneration {
                            new_key: agent_key,
                            new_key_signing_of_author: signature,
                        },
                        app_binding: AppBindingInput {
                            app_name: installed_app_id.clone(),
                            installed_app_id,
                            dna_hashes,
                            metadata: Default::default(), // TODO: pass in necessary metadata
                        },
                        derivation_details: Some(derivation),
                        create_only: false,
                    };

                    state.register_key(input).await?;
                }
            }

            app_result
        }

        /// Install DNAs and set up Cells as specified by an AppBundle
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn install_app_bundle(
            self: Arc<Self>,
            payload: InstallAppPayload,
        ) -> ConductorResult<InstalledApp> {
            let ignore_genesis_failure = payload.ignore_genesis_failure;

            let InstallAppPayload {
                source,
                agent_key,
                installed_app_id,
                membrane_proofs,
                existing_cells,
                network_seed,
                ..
            } = payload;

            let bundle = {
                let original_bundle = source.resolve().await?;
                if let Some(network_seed) = network_seed {
                    let mut manifest = original_bundle.manifest().to_owned();
                    manifest.set_network_seed(network_seed);
                    AppBundle::from(original_bundle.into_inner().update_manifest(manifest)?)
                } else {
                    original_bundle
                }
            };
            let manifest = bundle.manifest().clone();

            // Use deferred memproofs only if no memproofs are provided.
            // If a memproof map is provided, it will override the allow_deferred_memproofs setting,
            // and the provided memproofs will be used immediately.
            let defer_memproofs = match &manifest {
                AppManifest::V1(m) => m.allow_deferred_memproofs && membrane_proofs.is_none(),
            };

            let membrane_proofs = membrane_proofs.unwrap_or_default();

            let installed_app_id =
                installed_app_id.unwrap_or_else(|| manifest.app_name().to_owned());

            if installed_app_id == DPKI_APP_ID {
                return Err(ConductorError::Other(
                    "Can't install app with reserved id 'DPKI'"
                        .to_string()
                        .into(),
                ));
            }

            // NOTE: for testing with inline zomes when the conductor is restarted, it's
            //       essential that the installed_hash is included in the app manifest,
            //       so that the local DNAs with inline zomes can be loaded from
            //       local storage
            let local_dnas = self
                .ribosome_store()
                .share_ref(|store| bundle.get_all_dnas_from_store(store));

            let ops = bundle
                .resolve_cells(&local_dnas, membrane_proofs, existing_cells)
                .await?;

            self.clone()
                .install_app_common(
                    installed_app_id,
                    manifest,
                    agent_key,
                    defer_memproofs,
                    ops,
                    ignore_genesis_failure,
                )
                .await
        }

        /// Uninstall an app, removing all traces of it including its cells.
        ///
        /// This will fail if the app is depended upon by other apps via the UseExisting
        /// cell provisioning strategy, in which case the dependent app(s) would first need
        /// to be uninstalled, or the `force` param can be set to true.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
        pub async fn uninstall_app(
            self: Arc<Self>,
            installed_app_id: &InstalledAppId,
            force: bool,
        ) -> ConductorResult<()> {
            let deps = self
                .get_state()
                .await?
                .get_dependent_apps(installed_app_id, true)?;

            // Only uninstall the app if there are no protected dependents,
            // or if force is used
            if force || deps.is_empty() {
                let self_clone = self.clone();
                let app = self.remove_app_from_db(installed_app_id).await?;
                tracing::debug!(msg = "Removed app from db.", app = ?app);

                // Remove cells which may now be dangling due to the removed app
                self_clone
                    .process_app_status_fx(AppStatusFx::SpinDown, None)
                    .await?;

                let installed_app_ids = self
                    .get_state()
                    .await?
                    .installed_apps_and_services()
                    .iter()
                    .filter(|(app_id, _)| is_app(app_id))
                    .map(|(app_id, _)| app_id.clone())
                    .collect::<HashSet<_>>();
                self.app_broadcast.retain(installed_app_ids);

                Ok(())
            } else {
                Err(ConductorError::AppHasDependents(
                    installed_app_id.clone(),
                    deps,
                ))
            }
        }

        /// List active AppIds
        pub async fn list_running_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
            let state = self.get_state().await?;
            Ok(state
                .running_apps()
                .filter(|(id, _)| is_app(id))
                .map(|(id, _)| id)
                .cloned()
                .collect())
        }

        /// List Apps with their information
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_apps(
            &self,
            status_filter: Option<AppStatusFilter>,
        ) -> ConductorResult<Vec<AppInfo>> {
            use AppStatusFilter::*;
            let conductor_state = self.get_state().await?;

            let apps_ids: Vec<&String> = match status_filter {
                Some(Enabled) => conductor_state
                    .enabled_apps()
                    .filter(|(id, _)| is_app(id))
                    .map(|(id, _)| id)
                    .collect(),
                Some(Disabled) => conductor_state
                    .disabled_apps()
                    .filter(|(id, _)| is_app(id))
                    .map(|(id, _)| id)
                    .collect(),
                Some(Running) => conductor_state
                    .running_apps()
                    .filter(|(id, _)| is_app(id))
                    .map(|(id, _)| id)
                    .collect(),
                Some(Stopped) => conductor_state
                    .stopped_apps()
                    .filter(|(id, _)| is_app(id))
                    .map(|(id, _)| id)
                    .collect(),
                Some(Paused) => conductor_state
                    .paused_apps()
                    .filter(|(id, _)| is_app(id))
                    .map(|(id, _)| id)
                    .collect(),
                None => conductor_state
                    .installed_apps_and_services()
                    .keys()
                    .filter(|id| is_app(id))
                    .collect(),
            };

            let app_infos: Vec<AppInfo> = apps_ids
                .into_iter()
                .map(|app_id| self.get_app_info_inner(app_id, &conductor_state))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect();

            Ok(app_infos)
        }

        /// Get the IDs of all active installed Apps which use this Cell
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_running_apps_for_dependent_cell_id(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<HashSet<InstalledAppId>> {
            Ok(self
                .get_state()
                .await?
                .running_apps()
                .filter(|(_, v)| v.all_cells().any(|i| i == *cell_id))
                .map(|(k, _)| k)
                .cloned()
                .collect())
        }

        /// Find the ID of the first active installed App which uses this Cell
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn find_cell_with_role_alongside_cell(
            &self,
            cell_id: &CellId,
            role_name: &RoleName,
        ) -> ConductorResult<Option<CellId>> {
            Ok(self
                .get_state()
                .await?
                .running_apps()
                .find(|(_, running_app)| running_app.all_cells().any(|i| i == *cell_id))
                .and_then(|(_, running_app)| {
                    let app = running_app.clone().into_common();
                    app.role(role_name).ok().map(|role| match role {
                        AppRoleAssignment::Primary(primary) => {
                            CellId::new(primary.dna_hash().clone(), running_app.agent_key().clone())
                        }
                        AppRoleAssignment::Dependency(dependency) => dependency.cell_id.clone(),
                    })
                }))
        }

        /// Get the IDs of all active installed Apps which use this Dna
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
        ) -> ConductorResult<Option<AppInfo>> {
            let state = self.get_state().await?;
            let maybe_app_info = self.get_app_info_inner(installed_app_id, &state)?;
            Ok(maybe_app_info)
        }

        /// Run genesis for cells of an app which was installed using `allow_deferred_memproofs`
        pub async fn provide_memproofs(
            self: Arc<Self>,
            installed_app_id: &InstalledAppId,
            mut memproofs: MemproofMap,
        ) -> ConductorResult<()> {
            let state = self.get_state().await?;

            let app = state.get_app(installed_app_id)?;
            let cells_to_genesis = app
                .primary_roles()
                .map(|(role_name, role)| {
                    (
                        CellId::new(role.dna_hash().clone(), app.agent_key.clone()),
                        memproofs.remove(role_name),
                    )
                })
                .collect();

            crate::conductor::conductor::genesis_cells(self.clone(), cells_to_genesis).await?;

            self.update_state({
                let installed_app_id = installed_app_id.clone();
                move |mut state| {
                    let app = state.get_app_mut(&installed_app_id)?;
                    app.status =
                        AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs);
                    Ok(state)
                }
            })
            .await?;

            self.clone()
                .create_and_add_initialized_cells_for_running_apps(Some(installed_app_id))
                .await?;
            let app_ids: HashSet<_> = [installed_app_id.to_owned()].into_iter().collect();
            let delta = self
                .clone()
                .reconcile_app_status_with_cell_status(Some(app_ids.clone()))
                .await?;
            self.process_app_status_fx(delta, Some(app_ids)).await?;
            Ok(())
        }

        /// Update the agent key for an installed app
        // TODO: fully implement after DPKI is available
        #[allow(unused)]
        pub async fn rotate_app_agent_key(
            &self,
            installed_app_id: &InstalledAppId,
        ) -> ConductorResult<AgentPubKey> {
            // TODO: use key derivation for DPKI
            let new_agent_key = self.keystore().new_sign_keypair_random().await?;
            let ret = new_agent_key.clone();
            self.update_state({
                let installed_app_id = installed_app_id.clone();
                move |mut state| {
                    let app = state.get_app_mut(&installed_app_id)?;
                    app.agent_key = new_agent_key;
                    // TODO: update all cell IDs in the roles
                    Ok(state)
                }
            })
            .await?;
            unimplemented!("this is a partial implementation for reference only")
        }

        fn get_app_info_inner(
            &self,
            app_id: &InstalledAppId,
            state: &ConductorState,
        ) -> ConductorResult<Option<AppInfo>> {
            match state.get_app(app_id) {
                Err(_) => Ok(None),
                Ok(app) => {
                    let dna_definitions = self.get_dna_definitions(app)?;
                    Ok(Some(AppInfo::from_installed_app(app, &dna_definitions)))
                }
            }
        }
    }
}

/// Methods related to cell access
mod cell_impls {
    use std::collections::BTreeSet;

    use holochain_conductor_api::CompatibleCells;

    use super::*;

    impl Conductor {
        pub(crate) async fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<Arc<Cell>> {
            // Can only get a cell from the running_cells list
            if let Some(cell) = self.running_cells.share_ref(|c| c.get(cell_id).cloned()) {
                Ok(cell.cell)
            } else {
                // If not in running_cells list, check if the cell id is registered at all,
                // to give a different error message for disabled vs missing.
                let present = self
                    .get_state()
                    .await?
                    .installed_apps_and_services()
                    .values()
                    .flat_map(|app| app.all_cells())
                    .any(|id| id == *cell_id);
                if present {
                    Err(ConductorError::CellDisabled(cell_id.clone()))
                } else {
                    Err(ConductorError::CellMissing(cell_id.clone()))
                }
            }
        }

        /// Iterator over cells which are fully "live", meaning they have been
        /// fully initialized and are registered with the kitsune network layer.
        /// Generally used to handle conductor interface requests.
        ///
        /// If a cell is in `running_cells`, then it is "live".
        pub fn running_cell_ids(&self) -> HashSet<CellId> {
            self.running_cells
                .share_ref(|cells| cells.keys().cloned().collect())
        }

        /// Returns all installed cells which are forward compatible with the specified DNA,
        /// including direct matches, by examining the "lineage" specified by DNAs of currently installed cells.
        ///
        /// Each DnaDef specifies a "lineage" field of DNA hashes, which indicates that the DNA is forward-compatible
        /// with the DNAs specified in its lineage. If the DnaHash parameter is contained within the lineage of any
        /// installed cell's DNA, that cell will be returned in the result set, since it has declared
        /// itself forward-compatible.
        pub async fn cells_by_dna_lineage(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<CompatibleCells> {
            // TODO: OPTIMIZE: cache the DNA lineages
            Ok(self
                .get_state()
                .await?
                // Look in all installed apps
                .installed_apps_and_services()
                .values()
                .filter_map(|app| {
                    let cells_in_lineage: BTreeSet<_> = app
                        // Look in all cells for the app
                        .all_cells()
                        .filter_map(|cell_id| {
                            let cell_dna_hash = cell_id.dna_hash();
                            if cell_dna_hash == dna_hash {
                                // If a direct hit, include this CellId in the list of candidates
                                Some(cell_id.clone())
                            } else {
                                // If this cell *contains* the given DNA in *its* lineage, include it.
                                self.get_dna_def(cell_id.dna_hash())
                                    .map(|dna_def| dna_def.lineage.contains(dna_hash))
                                    .unwrap_or(false)
                                    .then(|| cell_id.clone())
                            }
                        })
                        .collect();
                    if cells_in_lineage.is_empty() {
                        None
                    } else {
                        Some((app.installed_app_id.clone(), cells_in_lineage))
                    }
                })
                .collect())
        }
    }
}

/// Methods related to clone cell management
mod clone_cell_impls {
    use holochain_zome_types::prelude::ClonedCell;

    use super::*;

    impl Conductor {
        /// Create a new cell in an existing app based on an existing DNA.
        ///
        /// Cells of an invalid agent key cannot be cloned.
        pub async fn create_clone_cell(
            self: Arc<Self>,
            installed_app_id: &InstalledAppId,
            payload: CreateCloneCellPayload,
        ) -> ConductorResult<ClonedCell> {
            let CreateCloneCellPayload {
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
            let app = state.get_app(installed_app_id)?;
            let app_role = app.primary_role(&role_name)?;
            // If base cell has been provisioned, check first in Deepkey if agent key is valid
            if app_role.is_provisioned {
                if let Some(dpki) = self.running_services().dpki {
                    let agent_key = app.agent_key().clone();
                    let key_state = dpki
                        .state()
                        .await
                        .key_state(agent_key.clone(), Timestamp::now())
                        .await?;
                    if let KeyState::Invalid(_) = key_state {
                        return Err(DpkiServiceError::DpkiAgentInvalid(
                            agent_key,
                            Timestamp::now(),
                        )
                        .into());
                    }
                }

                // Check source chain if agent key is valid
                let source_chain = SourceChain::new(
                    self.get_or_create_authored_db(app_role.dna_hash(), app.agent_key().clone())?,
                    self.get_or_create_dht_db(app_role.dna_hash())?,
                    self.get_or_create_space(app_role.dna_hash())?
                        .dht_query_cache,
                    self.keystore.clone(),
                    app.agent_key().clone(),
                )
                .await?;
                source_chain.valid_create_agent_key_action().await?;
            }

            // add cell to app
            let clone_cell = self
                .add_clone_cell_to_app(
                    installed_app_id.clone(),
                    role_name.clone(),
                    modifiers.serialized()?,
                    name,
                )
                .await?;

            // run genesis on cloned cell
            let cells = vec![(clone_cell.cell_id.clone(), membrane_proof)];
            crate::conductor::conductor::genesis_cells(self.clone(), cells).await?;
            self.create_and_add_initialized_cells_for_running_apps(Some(installed_app_id))
                .await?;
            Ok(clone_cell)
        }

        /// Disable a clone cell.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn disable_clone_cell(
            &self,
            installed_app_id: &InstalledAppId,
            DisableCloneCellPayload { clone_cell_id }: &DisableCloneCellPayload,
        ) -> ConductorResult<()> {
            let (_, removed_cell_id) = self
                .update_state_prime({
                    let app_id = installed_app_id.clone();
                    let clone_cell_id = clone_cell_id.to_owned();
                    move |mut state| {
                        let app = state.get_app_mut(&app_id)?;
                        let clone_id = app.get_clone_id(&clone_cell_id)?;
                        let dna_hash = app.get_clone_dna_hash(&clone_cell_id)?;
                        app.disable_clone_cell(&clone_id)?;
                        let cell_id = CellId::new(dna_hash, app.agent_key().clone());
                        Ok((state, cell_id))
                    }
                })
                .await?;
            self.remove_cells(&[removed_cell_id]).await;
            Ok(())
        }

        /// Enable a disabled clone cell.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn enable_clone_cell(
            self: Arc<Self>,
            installed_app_id: &InstalledAppId,
            payload: &EnableCloneCellPayload,
        ) -> ConductorResult<ClonedCell> {
            let conductor = self.clone();
            let (_, enabled_cell) = self
                .update_state_prime({
                    let app_id = installed_app_id.clone();
                    let clone_cell_id = payload.clone_cell_id.to_owned();
                    move |mut state| {
                        let app = state.get_app_mut(&app_id)?;
                        let clone_id = app.get_disabled_clone_id(&clone_cell_id)?;
                        let (cell_id, _) = app.enable_clone_cell(&clone_id)?.into_inner();
                        let app_role = app.primary_role(&clone_id.as_base_role_name())?;
                        let original_dna_hash = app_role.dna_hash().clone();
                        let ribosome = conductor.get_ribosome(cell_id.dna_hash())?;
                        let dna = ribosome.dna_file.dna();
                        let dna_modifiers = dna.modifiers.clone();
                        let name = dna.name.clone();
                        let enabled_cell = ClonedCell {
                            cell_id,
                            clone_id,
                            original_dna_hash,
                            dna_modifiers,
                            name,
                            enabled: true,
                        };
                        Ok((state, enabled_cell))
                    }
                })
                .await?;

            self.create_and_add_initialized_cells_for_running_apps(Some(installed_app_id))
                .await?;
            Ok(enabled_cell)
        }

        /// Delete a clone cell.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn delete_clone_cell(
            &self,
            DeleteCloneCellPayload {
                app_id,
                clone_cell_id,
            }: &DeleteCloneCellPayload,
        ) -> ConductorResult<()> {
            self.update_state_prime({
                let app_id = app_id.clone();
                let clone_cell_id = clone_cell_id.clone();
                move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    let clone_id = app.get_disabled_clone_id(&clone_cell_id)?;
                    app.delete_clone_cell(&clone_id)?;
                    Ok((state, ()))
                }
            })
            .await?;
            self.remove_dangling_cells().await?;
            Ok(())
        }
    }
}

/// Methods related to management of app and cell status
mod app_status_impls {
    use holochain_p2p::AgentPubKeyExt;

    use super::*;

    impl Conductor {
        /// Adjust which cells are present in the Conductor (adding and removing as
        /// needed) to match the current reality of all app statuses.
        /// - If a Cell is used by at least one Running app, then ensure it is added
        /// - If a Cell is used by no running apps, then ensure it is removed.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn create_and_add_initialized_cells_for_running_apps(
            self: Arc<Self>,
            app_id: Option<&InstalledAppId>,
        ) -> ConductorResult<CellStartupErrors> {
            let results = self.clone().create_cells_for_running_apps(app_id).await?;
            let (new_cells, errors): (Vec<_>, Vec<_>) =
                results.into_iter().partition(Result::is_ok);

            let new_cells: Vec<_> = new_cells
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

            // Add agents to local agent store in kitsune

            future::join_all(new_cells.iter().enumerate().map(|(i, (cell, _))| {
                let sleuth_id = self.config.sleuth_id();
                async move {
                    let p2p_agents_db = cell.p2p_agents_db().clone();
                    let cell_id = cell.id().clone();
                    let kagent = cell_id.agent_pubkey().to_kitsune();
                    let maybe_agent_info = p2p_agents_db.p2p_get_agent(&kagent).await.ok().flatten();
                    let maybe_initial_arq = maybe_agent_info.clone().map(|i| i.storage_arq);
                    let agent_pubkey = cell_id.agent_pubkey().clone();

                    let res = tokio::time::timeout(
                        JOIN_NETWORK_WAITING_PERIOD,
                        cell.holochain_p2p_dna().clone().join(
                            agent_pubkey,
                            maybe_agent_info,
                            maybe_initial_arq,
                        ),
                    )
                        .await;

                    match res {
                        Ok(r) => {
                            match r {
                                Ok(_) => {
                                    aitia::trace!(&hc_sleuth::Event::AgentJoined {
                                        node: sleuth_id,
                                        agent: cell_id.agent_pubkey().clone()
                                    });
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Network join failed for {cell_id}. This should never happen. Error: {e:?}"
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            tracing::warn!(
                                "Network join took longer than {JOIN_NETWORK_WAITING_PERIOD:?} for {cell_id}. Cell startup proceeding anyway."
                            );
                        }
                    }
                }.instrument(tracing::info_span!("network join task", ?i))
            }))
                .await;

            // Add the newly created cells to the Conductor
            self.add_and_initialize_cells(new_cells);

            Ok(errors)
        }

        /// Adjust app statuses (via state transitions) to match the current
        /// reality of which Cells are present in the conductor.
        /// - Do not change state for Disabled apps. For all others:
        /// - If an app is Paused but all of its (required) Cells are on,
        ///     then set it to Running
        /// - If an app is Running but at least one of its (required) Cells are off,
        ///     then set it to Paused
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
        pub(crate) async fn reconcile_app_status_with_cell_status(
            &self,
            app_ids: Option<HashSet<InstalledAppId>>,
        ) -> ConductorResult<AppStatusFx> {
            use AppStatus::*;
            use AppStatusTransition::*;

            // NOTE: this is checking all *live* cells, meaning all cells
            // which have fully joined the network. This could lead to a race condition
            // when an app is first starting up, it checks its cell status, and if
            // all cells haven't joined the network yet, the app will get disabled again.
            //
            // How this *should* be handled is that join retrying should be more frequent,
            // and should be sure to update app state on every newly joined cell, so that
            // the app will be enabled as soon as all cells are fully live. For now though,
            // we might consider relaxing this check so that this race condition isn't
            // possible, and let ourselves be optimistic that all cells will join soon after
            // the app starts.
            let cell_ids: HashSet<CellId> = self.running_cell_ids();
            let (_, delta) = self
                .update_state_prime(move |mut state| {
                    #[allow(deprecated)]
                    let apps =
                        state
                            .installed_apps_and_services_mut()
                            .iter_mut()
                            .filter(|(id, _)| {
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
                                        .filter(|id| !cell_ids.contains(id))
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
                                    if app.required_cells().all(|id| cell_ids.contains(&id)) {
                                        app.status.transition(Start)
                                    } else {
                                        AppStatusFx::NoChange
                                    }
                                }
                                Disabled(_) => {
                                    // Disabled status should never automatically change.
                                    AppStatusFx::NoChange
                                }
                                AwaitingMemproofs => AppStatusFx::NoChange,
                            }
                        })
                        .fold(AppStatusFx::default(), AppStatusFx::combine);
                    Ok((state, delta))
                })
                .await?;
            Ok(delta)
        }
    }
}

/// Methods related to management of Conductor state
mod service_impls {

    use super::*;

    impl Conductor {
        /// Access the current conductor services
        pub fn running_services(&self) -> ConductorServices {
            self.running_services.share_ref(|s| s.clone())
        }

        #[cfg(feature = "test_utils")]
        /// Access the current conductor services mutably
        pub fn running_services_mutex(&self) -> &RwShare<ConductorServices> {
            &self.running_services
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn initialize_services(self: Arc<Self>) -> ConductorResult<()> {
            self.initialize_service_dpki().await?;
            Ok(())
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn initialize_service_dpki(self: Arc<Self>) -> ConductorResult<()> {
            if let Some(installation) = self.get_state().await?.conductor_services.dpki {
                self.running_services.share_mut(|s| {
                    s.dpki = Some(Arc::new(DpkiService::new_deepkey(
                        installation,
                        self.clone(),
                    )));
                });
            }
            Ok(())
        }

        /// Install the DPKI service using the given Deepkey DNA.
        /// Note, this currently is done automatically when the conductor is first initialized,
        /// using the DpkiConfig in the conductor config. We may also provide this as an admin
        /// method some day.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn install_dpki(
            self: Arc<Self>,
            dna: DnaFile,
            enable: bool,
        ) -> ConductorResult<()> {
            let dna_hash = dna.dna_hash().clone();

            self.register_dna(dna.clone()).await?;

            let device_seed_lair_tag = if let Some(tag) =
                self.get_config().device_seed_lair_tag.clone()
            {
                tag
            } else {
                return Err(ConductorError::other("DPKI could not be installed because `device_seed_lair_tag` is not set in the conductor config. If using DPKI, a device seed must be created in lair, and the tag specified in the conductor config."));
            };

            let derivation_path = [0].into();
            let dst_tag = format!("{device_seed_lair_tag}.0");

            let seed_info = self
                .keystore()
                .lair_client()
                .derive_seed(
                    device_seed_lair_tag.clone().into(),
                    None,
                    dst_tag.into(),
                    None,
                    derivation_path,
                )
                .await?;

            // The initial agent key is the first derivation from the device seed.
            // Updated DPKI agent keys are sequential derivations from the same device seed.
            let agent = holo_hash::AgentPubKey::from_raw_32(seed_info.ed25519_pub_key.0.to_vec());
            let cell_id = CellId::new(dna_hash.clone(), agent.clone());

            self.clone()
                .install_app_minimal(DPKI_APP_ID.into(), Some(agent), &[(dna, None)])
                .await?;

            // In multi-conductor tests, we often want to delay enabling DPKI until all conductors
            // have exchanged peer info, so that the initial DPKI publish can go more smoothly.
            if enable {
                self.clone().enable_app(DPKI_APP_ID.into()).await?;
            }

            // Ensure that the space is created for DPKI, in case it's not enabled
            self.spaces.get_or_create_space(&dna_hash)?;

            assert!(self
                .spaces
                .get_from_spaces(|s| (*s.dna_hash).clone())
                .contains(&dna_hash));

            let installation = DeepkeyInstallation {
                cell_id,
                device_seed_lair_tag,
            };
            self.update_state(move |mut state| {
                state.conductor_services.dpki = Some(installation);
                Ok(state)
            })
            .await?;

            self.clone().initialize_service_dpki().await?;

            if enable {
                if let Ok(Some(info)) = self.get_app_info(&DPKI_APP_ID.into()).await {
                    assert_eq!(info.status, holochain_conductor_api::AppInfoStatus::Running);
                } else {
                    panic!("DPKI service not installed!");
                }
            }

            Ok(())
        }
    }
}

/// Methods related to management of Conductor state
mod state_impls {
    use super::*;

    impl Conductor {
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn get_state(&self) -> ConductorResult<ConductorState> {
            self.spaces.get_state().await
        }

        /// Update the internal state with a pure function mapping old state to new
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn update_state<F>(&self, f: F) -> ConductorResult<ConductorState>
        where
            F: Send + FnOnce(ConductorState) -> ConductorResult<ConductorState> + 'static,
        {
            self.spaces.update_state(f).await
        }

        /// Update the internal state with a pure function mapping old state to new,
        /// which may also produce an output value which will be the output of
        /// this function
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
        /// So ideally this would be called ONCE per conductor lifecycle ONLY.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
        pub(crate) async fn start_scheduler(
            self: Arc<Self>,
            interval_period: std::time::Duration,
        ) -> StateMutationResult<()> {
            // Clear all ephemeral cruft in all cells before starting a scheduler.
            let tasks = self
                .spaces
                .get_from_spaces(|space| {
                    let all_dbs = space.get_all_authored_dbs();

                    all_dbs.into_iter().map(|db| async move {
                        db.write_async(delete_all_ephemeral_scheduled_fns).await
                    })
                })
                .into_iter()
                .flatten();

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

            Ok(())
        }

        /// The scheduler wants to dispatch any functions that are due.
        pub(crate) async fn dispatch_scheduled_fns(self: Arc<Self>, now: Timestamp) {
            let cell_arcs = {
                let mut cell_arcs = vec![];
                for cell_id in self.running_cell_ids() {
                    if let Ok(cell_arc) = self.cell_by_id(&cell_id).await {
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
    use std::sync::atomic::Ordering;

    use holochain_zome_types::action::builder;

    use super::*;

    impl Conductor {
        /// Grant a zome call capability for a cell
        pub async fn grant_zome_call_capability(
            &self,
            payload: GrantZomeCallCapabilityPayload,
        ) -> ConductorApiResult<ActionHash> {
            let GrantZomeCallCapabilityPayload { cell_id, cap_grant } = payload;

            // Must init before committing a grant
            let cell = self.cell_by_id(&cell_id).await?;
            cell.check_or_run_zome_init().await?;

            let source_chain = SourceChain::new(
                self.get_or_create_authored_db(
                    cell_id.dna_hash(),
                    cell.id().agent_pubkey().clone(),
                )?,
                self.get_or_create_dht_db(cell_id.dna_hash())?,
                self.get_or_create_space(cell_id.dna_hash())?
                    .dht_query_cache,
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

            let action_hash = source_chain
                .put_weightless(
                    action_builder,
                    Some(cap_grant_entry),
                    ChainTopOrdering::default(),
                )
                .await?;

            let cell = self.cell_by_id(&cell_id).await?;
            source_chain.flush(cell.holochain_p2p_dna()).await?;

            Ok(action_hash)
        }

        /// Create a JSON dump of the cell's state
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
            let cell = self.cell_by_id(cell_id).await?;
            let authored_db = cell.get_or_create_authored_db()?;
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
                integration_dump: integration_dump(dht_db).await?,
            };
            // Add summary
            let summary = out.to_string();
            let out = (out, summary);
            Ok(serde_json::to_string_pretty(&out)?)
        }

        /// Create a JSON dump of the conductor's state
        pub async fn dump_conductor_state(&self) -> ConductorApiResult<String> {
            #[derive(Serialize, Debug)]
            pub struct ConductorSerialized {
                running_cells: Vec<(DnaHashB64, AgentPubKeyB64)>,
                shutting_down: bool,
                admin_websocket_ports: Vec<u16>,
                app_interfaces: Vec<AppInterfaceId>,
            }

            #[derive(Serialize, Debug)]
            struct ConductorDump {
                conductor: ConductorSerialized,
                state: ConductorState,
            }

            let conductor_state = self.get_state().await?;

            let conductor = ConductorSerialized {
                running_cells: self.running_cells.share_ref(|c| {
                    c.clone()
                        .into_keys()
                        .map(|id| {
                            let (dna, agent) = id.into_dna_and_agent();
                            (dna.into(), agent.into())
                        })
                        .collect()
                }),
                shutting_down: self.shutting_down.load(Ordering::SeqCst),
                admin_websocket_ports: self.admin_websocket_ports.share_ref(|p| p.clone()),
                app_interfaces: conductor_state.app_interfaces.keys().cloned().collect(),
            };

            let dump = ConductorDump {
                conductor,
                state: conductor_state,
            };

            let out = serde_json::to_string_pretty(&dump)?;

            Ok(out)
        }

        /// Create a comprehensive structured dump of a cell's state
        pub async fn dump_full_cell_state(
            &self,
            cell_id: &CellId,
            dht_ops_cursor: Option<u64>,
        ) -> ConductorApiResult<FullStateDump> {
            let authored_db =
                self.get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())?;
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

        /// JSON dump of backend network stats
        pub async fn dump_network_stats(&self) -> ConductorApiResult<String> {
            use holochain_p2p::HolochainP2pSender;
            self.holochain_p2p()
                .dump_network_stats()
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
    use tokio::sync::broadcast;

    impl Conductor {
        pub(crate) fn ribosome_store(&self) -> &RwShare<RibosomeStore> {
            &self.ribosome_store
        }

        pub(crate) fn get_queue_consumer_workflows(&self) -> QueueConsumerMap {
            self.spaces.queue_consumer_map.clone()
        }

        /// Get a signal broadcast sender for a cell.
        pub async fn get_signal_tx(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<broadcast::Sender<Signal>> {
            let app = self
                .find_app_containing_cell(cell_id)
                .await?
                .ok_or_else(|| ConductorError::CellMissing(cell_id.clone()))?;

            Ok(self.app_broadcast.create_send_handle(app.id().clone()))
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
        pub(crate) fn get_or_create_space(&self, dna_hash: &DnaHash) -> DatabaseResult<Space> {
            self.spaces.get_or_create_space(dna_hash)
        }

        pub(crate) fn get_or_create_authored_db(
            &self,
            dna_hash: &DnaHash,
            author: AgentPubKey,
        ) -> DatabaseResult<DbWrite<DbKindAuthored>> {
            self.spaces.get_or_create_authored_db(dna_hash, author)
        }

        pub(crate) fn get_or_create_dht_db(
            &self,
            dna_hash: &DnaHash,
        ) -> DatabaseResult<DbWrite<DbKindDht>> {
            self.spaces.dht_db(dna_hash)
        }

        pub(crate) fn get_or_create_cache_db(
            &self,
            dna_hash: &DnaHash,
        ) -> DatabaseResult<DbWrite<DbKindCache>> {
            self.spaces.cache(dna_hash)
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

        #[cfg(feature = "test_utils")]
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

        /// Get a TaskManagerClient
        pub fn task_manager(&self) -> TaskManagerClient {
            self.task_manager.clone()
        }

        /// Find the app which contains the given cell by its [CellId].
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn find_app_containing_cell(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<Option<InstalledApp>> {
            Ok(self
                .get_state()
                .await?
                .find_app_containing_cell(cell_id)
                .cloned())
        }
    }
}

mod authenticate_token_impls {
    use super::*;
    use holochain_conductor_api::{
        AppAuthenticationToken, AppAuthenticationTokenIssued, IssueAppAuthenticationTokenPayload,
    };

    impl Conductor {
        /// Issue a new app interface authentication token for the given `installed_app_id`.
        pub fn issue_app_authentication_token(
            &self,
            payload: IssueAppAuthenticationTokenPayload,
        ) -> ConductorResult<AppAuthenticationTokenIssued> {
            let (token, expires_at) = self.app_auth_token_store.share_mut(|app_connection_auth| {
                app_connection_auth.issue_token(
                    payload.installed_app_id,
                    payload.expiry_seconds,
                    payload.single_use,
                )
            });

            Ok(AppAuthenticationTokenIssued {
                token,
                expires_at: expires_at
                    .and_then(|i| i.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| Timestamp::saturating_from_dur(&d)),
            })
        }

        /// Revoke an app interface authentication token.
        pub fn revoke_app_authentication_token(
            &self,
            token: AppAuthenticationToken,
        ) -> ConductorResult<()> {
            self.app_auth_token_store
                .share_mut(|app_connection_auth| app_connection_auth.revoke_token(token));

            Ok(())
        }

        /// Authenticate the app interface authentication `token`, optionally requiring the token to
        /// have been issued for a specific `app_id`.
        ///
        /// Returns the [InstalledAppId] that the token was issued for.
        pub fn authenticate_app_token(
            &self,
            token: Vec<u8>,
            app_id: Option<InstalledAppId>,
        ) -> ConductorResult<InstalledAppId> {
            self.app_auth_token_store.share_mut(|app_connection_auth| {
                app_connection_auth.authenticate_token(token, app_id)
            })
        }
    }
}

#[async_trait::async_trait]
impl holochain_conductor_services::CellRunner for Conductor {
    async fn call_zome(
        &self,
        provenance: &AgentPubKey,
        cap_secret: Option<CapSecret>,
        cell_id: CellId,
        zome_name: ZomeName,
        fn_name: FunctionName,
        payload: ExternIO,
    ) -> anyhow::Result<ExternIO> {
        let now = Timestamp::now();
        let (nonce, expires_at) =
            holochain_nonce::fresh_nonce(now).map_err(ConductorApiError::Other)?;
        let call_unsigned = ZomeCallUnsigned {
            cell_id,
            zome_name,
            fn_name,
            cap_secret,
            provenance: provenance.clone(),
            payload,
            nonce,
            expires_at,
        };
        let call = ZomeCall::try_from_unsigned_zome_call(self.keystore(), call_unsigned).await?;
        let response = self.call_zome(call).await;
        match response {
            Ok(Ok(ZomeCallResponse::Ok(bytes))) => Ok(bytes),
            Ok(Ok(other)) => Err(anyhow::anyhow!(other.clone())),
            Ok(Err(error)) => Err(error.into()),
            Err(error) => Err(error.into()),
        }
    }
}

/// Private methods, only used within the Conductor, never called from outside.
impl Conductor {
    fn add_admin_port(&self, port: u16) {
        self.admin_websocket_ports.share_mut(|p| p.push(port));
    }

    /// Add fully constructed cells to the cell map in the Conductor
    #[allow(deprecated)]
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn add_and_initialize_cells(&self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (new_cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        self.running_cells.share_mut(|cells| {
            for cell in new_cells {
                let cell_id = cell.id().clone();
                tracing::debug!(?cell_id, "added cell");
                cells.insert(
                    cell_id,
                    CellItem {
                        cell: Arc::new(cell),
                        status: CellStatus::Joined,
                    },
                );
            }
        });
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }

    /// Remove all Cells which are not referenced by any Enabled app.
    /// (Cells belonging to Paused apps are not considered "dangling" and will not be removed).
    ///
    /// Additionally, if the cell is being removed because the last app referencing it was uninstalled,
    /// all data used by that cell (across Authored, DHT, and Cache databases) will also be removed.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn remove_dangling_cells(&self) -> ConductorResult<()> {
        let state = self.get_state().await?;

        let keepers: HashSet<CellId> = state
            .enabled_apps_and_services()
            .flat_map(|(_, app)| app.all_cells().collect::<HashSet<_>>())
            .collect();

        let all_cells: HashSet<CellId> = state
            .installed_apps_and_services()
            .iter()
            .flat_map(|(_, app)| app.all_cells().collect::<HashSet<_>>())
            .collect();

        let all_dnas: HashSet<_> = all_cells.iter().map(|cell_id| cell_id.dna_hash()).collect();

        // Clean up all cells that will be dropped (leave network, etc.)
        let cells_to_cleanup: Vec<_> = self.running_cells.share_mut(|cells| {
            let to_remove: Vec<_> = cells
                .keys()
                .filter(|id| !keepers.contains(id))
                .cloned()
                .collect();

            // remove all but the keepers
            to_remove
                .iter()
                .filter_map(|cell_id| cells.remove(cell_id))
                .map(|item| item.cell)
                .collect()
        });

        if !cells_to_cleanup.is_empty() {
            tracing::debug!(?cells_to_cleanup, "Cleaning up cells");
        }

        // Stop all long-running tasks for cells about to be dropped
        for cell in cells_to_cleanup.iter() {
            cell.cleanup().await?;
        }

        // Find any cleaned up cells which are no longer used by any app,
        // so that we can remove their data from the databases.
        let cells_to_purge: Vec<_> = cells_to_cleanup
            .iter()
            .filter_map(|cell| (!all_cells.contains(cell.id())).then_some(cell.id().clone()))
            .collect();

        // Find any DNAs from cleaned up cells which don't have representation in any cells
        // in any installed app, so that we can remove their data from the databases.
        let dnas_to_purge: Vec<_> = cells_to_cleanup
            .iter()
            .map(|cell| cell.id().dna_hash())
            .filter(|dna| !all_dnas.contains(dna))
            .collect();

        if !cells_to_purge.is_empty() {
            tracing::info!(?cells_to_purge, "Purging cells");
        }
        if !dnas_to_purge.is_empty() {
            tracing::info!(?dnas_to_purge, "Purging DNAs");
        }

        // Delete all data from authored databases which are longer installed
        for cell_id in cells_to_purge {
            let db = self
                .spaces
                .get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())?;
            let mut path = db.path().clone();
            if let Err(err) = ffs::remove_file(&path).await {
                tracing::warn!(?err, "Could not remove primary DB file, probably because it is still in use. Purging all data instead.");
                db.write_async(purge_data).await?;
            }
            path.set_extension("");
            let stem = path.to_string_lossy();
            for ext in ["db-shm", "db-wal"] {
                let path = PathBuf::from(format!("{stem}.{ext}"));
                if let Err(err) = ffs::remove_file(&path).await {
                    let err = err.remove_backtrace();
                    tracing::warn!(?err, "Failed to remove DB file");
                }
            }
        }

        // For any DNAs no longer represented in any installed app,
        // purge data from those DNA-specific databases
        for dna_hash in dnas_to_purge {
            futures::future::join_all(
                [
                    self.spaces
                        .dht_db(dna_hash)
                        .unwrap()
                        .write_async(purge_data)
                        .boxed(),
                    self.spaces
                        .cache(dna_hash)
                        .unwrap()
                        .write_async(purge_data)
                        .boxed(),
                    // TODO: also delete stale Wasms
                ]
                .into_iter(),
            )
            .await
            .into_iter()
            .collect::<Result<Vec<()>, _>>()?;
        }

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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    #[allow(clippy::complexity)]
    async fn create_cells_for_running_apps(
        self: Arc<Self>,
        app_id: Option<&InstalledAppId>,
    ) -> ConductorResult<Vec<Result<(Cell, InitialQueueTriggers), (CellId, CellError)>>> {
        // Closure for creating all cells in an app
        let state = self.get_state().await?;

        let app_cells: HashSet<CellId> = match app_id {
            Some(app_id) => {
                let app = state.get_app(app_id)?;
                if app.status().is_running() {
                    app.all_enabled_cells().collect()
                } else {
                    HashSet::new()
                }
            }
            None =>
            // Collect all CellIds across all apps, deduped
            {
                state
                    .installed_apps_and_services()
                    .iter()
                    .filter(|(_, app)| app.status().is_running())
                    .flat_map(|(_id, app)| app.all_enabled_cells())
                    .collect()
            }
        };

        // calculate the existing cells so we can filter those out, only creating
        // cells for CellIds that don't have cells
        let on_cells: HashSet<CellId> = self
            .running_cells
            .share_ref(|c| c.keys().cloned().collect());

        let tasks = app_cells.difference(&on_cells).map(|cell_id| {
            let handle = self.clone();
            let chc = handle.get_chc(cell_id);
            async move {
                let holochain_p2p_cell =
                    handle.holochain_p2p.to_dna(cell_id.dna_hash().clone(), chc);

                let space = handle
                    .get_or_create_space(cell_id.dna_hash())
                    .map_err(|e| CellError::FailedToCreateDnaSpace(ConductorError::from(e).into()))
                    .map_err(|err| (cell_id.clone(), err))?;

                let signal_tx = handle
                    .get_signal_tx(cell_id)
                    .await
                    .map_err(|err| (cell_id.clone(), CellError::ConductorError(Box::new(err))))?;

                tracing::info!(?cell_id, "Creating a cell");
                Cell::create(
                    cell_id.clone(),
                    handle,
                    space,
                    holochain_p2p_cell,
                    signal_tx,
                )
                .in_current_span()
                .await
                .map_err(|err| (cell_id.clone(), err))
            }
        });

        // Join on all apps and return a list of
        // apps that had successfully created cells
        // and any apps that encounted errors
        Ok(futures::future::join_all(tasks).await)
    }

    /// Deal with the side effects of an app status state transition
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
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
                Error(err) => return Err(ConductorError::AppStatusError(err)),
            };
        }

        Ok(last.1)
    }

    /// Entirely remove an app from the database, returning the removed app.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn add_clone_cell_to_app(
        &self,
        app_id: InstalledAppId,
        role_name: RoleName,
        dna_modifiers: DnaModifiersOpt,
        name: Option<String>,
    ) -> ConductorResult<ClonedCell> {
        let ribosome_store = &self.ribosome_store;
        // retrieve base cell DNA hash from conductor
        let (_, base_cell_dna_hash) = self
            .update_state_prime({
                let app_id = app_id.clone();
                let role_name = role_name.clone();
                move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    let app_role = app.primary_role(&role_name)?;
                    if app_role.is_clone_limit_reached() {
                        return Err(ConductorError::AppError(AppError::CloneLimitExceeded(
                            app_role.clone_limit(),
                            app_role.clone(),
                        )));
                    }
                    let original_dna_hash = app_role.dna_hash().clone();
                    Ok((state, original_dna_hash))
                }
            })
            .await?;
        let original_dna_hash = base_cell_dna_hash.clone();

        // clone cell from base cell DNA
        let clone_dna = ribosome_store.share_ref(|rs| {
            let mut dna_file = rs
                .get_dna_file(&base_cell_dna_hash)
                .ok_or(DnaError::DnaMissing(base_cell_dna_hash))?
                .update_modifiers(dna_modifiers);
            if let Some(name) = name {
                dna_file = dna_file.set_name(name);
            }
            Ok::<_, DnaError>(dna_file)
        })?;
        let name = clone_dna.dna().name.clone();
        let dna_modifiers = clone_dna.dna().modifiers.clone();
        let clone_dna_hash = clone_dna.dna_hash().to_owned();

        // add clone cell to app and instantiate resulting clone cell
        let (_, installed_clone_cell) = self
            .update_state_prime(move |mut state| {
                let state_copy = state.clone();
                let app = state.get_app_mut(&app_id)?;
                let agent_key = app.agent_key().to_owned();
                let clone_cell_id = CellId::new(clone_dna_hash, agent_key);

                // if cell id of new clone cell already exists, reject as duplicate
                if state_copy
                    .installed_apps_and_services()
                    .iter()
                    .flat_map(|(_, app)| app.all_cells())
                    .any(|cell_id| cell_id == clone_cell_id)
                {
                    return Err(ConductorError::AppError(AppError::DuplicateCellId(
                        clone_cell_id,
                    )));
                }

                let clone_id = app.add_clone(&role_name, clone_cell_id.dna_hash())?;
                let installed_clone_cell = ClonedCell {
                    cell_id: clone_cell_id,
                    clone_id,
                    original_dna_hash,
                    dna_modifiers,
                    name,
                    enabled: true,
                };
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
    use tokio::sync::broadcast;

    impl Conductor {
        pub async fn get_state_from_handle(&self) -> ConductorResult<ConductorState> {
            self.get_state().await
        }

        pub fn subscribe_to_app_signals(
            &self,
            installed_app_id: InstalledAppId,
        ) -> broadcast::Receiver<Signal> {
            self.app_broadcast.subscribe(installed_app_id)
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

        pub async fn get_cache_db(
            &self,
            cell_id: &CellId,
        ) -> ConductorApiResult<DbWrite<DbKindCache>> {
            let cell = self.cell_by_id(cell_id).await?;
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

        pub async fn get_cell_triggers(
            &self,
            cell_id: &CellId,
        ) -> ConductorApiResult<QueueTriggers> {
            let cell = self.cell_by_id(cell_id).await?;
            Ok(cell.triggers().clone())
        }
    }
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
fn purge_data(txn: &mut Transaction) -> DatabaseResult<()> {
    txn.execute("DELETE FROM DhtOp", ())?;
    txn.execute("DELETE FROM Action", ())?;
    txn.execute("DELETE FROM Entry", ())?;
    txn.execute("DELETE FROM ValidationReceipt", ())?;
    txn.execute("DELETE FROM ChainLock", ())?;
    txn.execute("DELETE FROM ScheduledFunctions", ())?;
    Ok(())
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
        let conductor = conductor.clone();
        let cell_id_inner = cell_id.clone();
        tokio::spawn(async move {
            let space = conductor
                .get_or_create_space(cell_id_inner.dna_hash())
                .map_err(|e| CellError::FailedToCreateDnaSpace(ConductorError::from(e).into()))?;

            let authored_db =
                space.get_or_create_authored_db(cell_id_inner.agent_pubkey().clone())?;
            let dht_db = space.dht_db;
            let dht_db_cache = space.dht_query_cache;
            let chc = conductor.get_chc(&cell_id_inner);
            let ribosome = conductor
                .get_ribosome(cell_id_inner.dna_hash())
                .map_err(Box::new)?;

            Cell::genesis(
                cell_id_inner.clone(),
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
        .map(|genesis_result| (cell_id, genesis_result.and_then(|r| r)))
    });
    let (_success, errors): (Vec<CellId>, Vec<(CellId, CellError)>) =
        futures::future::join_all(cells_tasks)
            .await
            .into_iter()
            .partition_map(|(cell_id, r)| match r {
                Ok(()) => either::Either::Left(cell_id),
                Err(err) => either::Either::Right((cell_id, err)),
            });

    // TODO: Reference count the databases successfully created here and clean them up on error.

    // If there were errors, cleanup and return the errors
    if !errors.is_empty() {
        Err(ConductorError::GenesisFailed { errors })
    } else {
        Ok(())
    }
}

/// Get the DPKI DNA from the filesystem or use the built-in one.
pub(crate) async fn get_dpki_dna(config: &DpkiConfig) -> DnaResult<DnaBundle> {
    if let Some(dna_path) = config.dna_path.as_ref() {
        DnaBundle::read_from_file(dna_path).await
    } else {
        DnaBundle::decode(holochain_deepkey_dna::DEEPKEY_DNA_BUNDLE_BYTES)
    }
}

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, and arbitrary role names.
/// Allows setting the clone_limit for every DNA.
pub fn app_manifest_from_dnas(
    dnas_with_roles: &[impl DnaWithRole],
    clone_limit: u32,
    memproofs_deferred: bool,
) -> AppManifest {
    let roles: Vec<_> = dnas_with_roles
        .iter()
        .map(|dr| {
            let dna = dr.dna();
            let path = PathBuf::from(format!("{}", dna.dna_hash()));
            let modifiers = DnaModifiersOpt::none();
            AppRoleManifest {
                name: dr.role(),
                dna: AppRoleDnaManifest {
                    location: Some(DnaLocation::Bundled(path.clone())),
                    modifiers,
                    installed_hash: Some(dr.dna().dna_hash().clone().into()),
                    clone_limit,
                },
                provisioning: Some(CellProvisioning::Create { deferred: false }),
            }
        })
        .collect();

    AppManifestCurrentBuilder::default()
        .name("[generated]".into())
        .description(None)
        .roles(roles)
        .allow_deferred_memproofs(memproofs_deferred)
        .build()
        .unwrap()
        .into()
}

/// Dump the integration json state.
pub async fn integration_dump<Db: ReadAccess<DbKindDht>>(
    vault: &Db,
) -> ConductorApiResult<IntegrationStateDump> {
    vault
        .read_async(move |txn| {
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
        .read_async(move |txn| {
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
        Some(cursor) => format!("{} AND DhtOp.rowid > {}", stmt_str, cursor),
        None => stmt_str.into(),
    };

    let mut stmt = txn.prepare(final_stmt_str.as_str())?;

    let r: Vec<DhtOp> = stmt
        .query_and_then([], |row| {
            holochain_state::query::map_sql_dht_op(false, "dht_type", row)
        })?
        .collect::<StateQueryResult<Vec<_>>>()?;
    Ok(r)
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(p2p_evt, handle)))]
async fn p2p_event_task(
    p2p_evt: holochain_p2p::event::HolochainP2pEventReceiver,
    handle: ConductorHandle,
) {
    /// The number of events we allow to run in parallel before
    /// starting to await on the join handles.
    const NUM_PARALLEL_EVTS: usize = 512;
    let num_tasks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_time = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let duration_metric = create_p2p_event_duration_metric();
    p2p_evt
        .for_each_concurrent(NUM_PARALLEL_EVTS, |evt| {
            let handle = handle.clone();
            let num_tasks = num_tasks.clone();
            let max_time = max_time.clone();
            let duration_metric = duration_metric.clone();
            async move {
                // Track whether the concurrency limit has been reached and keep the start time for reporting if so.
                let start = Instant::now();
                let current_num_tasks = num_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                let evt_dna_hash = evt.dna_hash().clone();

                // This loop is critical, ensure that nothing in the dispatch kills it by blocking permanently
                match tokio::time::timeout(std::time::Duration::from_secs(30), handle.dispatch_holochain_p2p_event(evt)).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::error!(
                                message = "error dispatching network event",
                                error = ?e,
                            );
                    }
                    Err(_) => {
                        tracing::error!("timeout while dispatching network event");
                    }
                }

                if current_num_tasks >= NUM_PARALLEL_EVTS {
                    let el = start.elapsed();
                    let us = el.as_micros() as u64;
                    let max_us = max_time
                        .fetch_max(us, std::sync::atomic::Ordering::Relaxed)
                        .max(us);

                    let s = tracing::info_span!("holochain_perf", this_event_time = ?el, max_event_micros = %max_us);
                    s.in_scope(|| tracing::info!("dispatch_holochain_p2p_event is saturated"))
                } else {
                    max_time.store(0, std::sync::atomic::Ordering::Relaxed);
                }

                duration_metric.record(start.elapsed().as_secs_f64(), &[
                    opentelemetry_api::KeyValue::new("dna_hash", format!("{:?}", evt_dna_hash)),
                ]);

                num_tasks.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
                .in_current_span()
        })
        .await;

    tracing::info!("p2p_event_task has ended");
}
