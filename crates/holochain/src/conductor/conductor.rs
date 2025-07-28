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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use futures::future;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use futures::stream::StreamExt;
#[cfg(feature = "wasmer_sys")]
use holochain_wasmer_host::module::ModuleCache;
use indexmap::IndexMap;
use itertools::Itertools;
use rusqlite::Transaction;
use tokio::sync::mpsc::error::SendError;
use tokio::task::JoinHandle;
use tracing::*;

pub use builder::*;
use holo_hash::DnaHash;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AppInfo;
use holochain_conductor_api::AppStatusFilter;
use holochain_conductor_api::FullIntegrationStateDump;
use holochain_conductor_api::FullStateDump;
use holochain_conductor_api::IntegrationStateDump;
use holochain_conductor_api::PeerMetaInfo;
use holochain_keystore::lair_keystore::spawn_lair_keystore;
use holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::sql::sql_cell::state_dump;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_state::nonce::witness_nonce;
use holochain_state::nonce::WitnessNonceResult;
use holochain_state::prelude::*;
use holochain_state::source_chain;
pub use holochain_types::share;
use holochain_zome_types::prelude::{AppCapGrantInfo, ClonedCell, Signature, Timestamp};
use kitsune2_api::AgentInfoSigned;

use crate::conductor::cell::Cell;
use crate::conductor::conductor::app_auth_token_store::AppAuthTokenStore;
use crate::conductor::conductor::app_broadcast::AppBroadcast;
use crate::conductor::config::ConductorConfig;
use crate::conductor::error::ConductorResult;
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
use super::config::AdminInterfaceConfig;
use super::config::InterfaceDriver;
use super::entry_def_store::get_entry_defs;
use super::error::ConductorError;
use super::interface::error::InterfaceResult;
use super::interface::websocket::spawn_admin_interface_tasks;
use super::interface::websocket::spawn_app_interface_task;
use super::interface::websocket::spawn_websocket_listener;
use super::manager::TaskManagerResult;
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

mod hc_p2p_handler_impl;

mod state_dump_helpers;

/// Verify signature of a signed zome call.
///
/// [Signature verification](holochain_conductor_api::AppRequest::CallZome)
pub(crate) mod zome_call_signature_verification;

pub(crate) mod app_broadcast;

#[cfg(test)]
pub(crate) mod tests;

/// Cloneable reference to a Conductor
pub type ConductorHandle = Arc<Conductor>;

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

#[allow(dead_code)]
pub(crate) type StopBroadcaster = task_motel::StopBroadcaster;
pub(crate) type StopReceiver = task_motel::StopListener;

/// A Conductor is a group of [Cell]s
pub struct Conductor {
    /// The collection of available, running cells associated with this Conductor
    running_cells: RwShare<IndexMap<CellId, Arc<Cell>>>,

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
    holochain_p2p: holochain_p2p::actor::DynHcP2p,

    post_commit: tokio::sync::mpsc::Sender<PostCommitArgs>,

    scheduler: Arc<parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Cache for wasmer modules, both on disk and in memory.
    ///
    /// This cache serves as a central storage location for wasmer modules,
    /// shared across all ribosomes. The cache is optional and can be disabled by
    /// setting it to `None`.
    ///
    /// Note: When using the `wasmer_wamr` feature, it's recommended to disable
    /// this cache since modules are interpreted at runtime rather than compiled,
    /// making caching unnecessary.
    pub(crate) wasmer_module_cache: Option<Arc<ModuleCacheLock>>,

    app_auth_token_store: RwShare<AppAuthTokenStore>,

    /// Container to connect app signals to app interfaces, by installed app id.
    app_broadcast: AppBroadcast,
}

impl std::fmt::Debug for Conductor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conductor").finish()
    }
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
            holochain_p2p: holochain_p2p::actor::DynHcP2p,
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
                running_cells: RwShare::new(IndexMap::new()),
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
                #[cfg(feature = "wasmer_sys")]
                wasmer_module_cache: Some(Arc::new(ModuleCacheLock::new(ModuleCache::new(
                    maybe_data_root_path.map(|p| p.join(WASM_CACHE)),
                )))),
                #[cfg(feature = "wasmer_wamr")]
                wasmer_module_cache: None,
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

            let mut tm = self.task_manager();
            let task = self.detach_task_management().expect("Attempting to shut down after already detaching task management or previous shutdown");
            tokio::task::spawn(async move {
                tracing::info!("Sending shutdown signal to all managed tasks.");
                let (_, r) = futures::join!(tm.shutdown().boxed(), task,);
                r?
            })
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(scope=self.config.network.tracing_scope)))]
        pub(crate) async fn initialize_conductor(
            self: Arc<Self>,
            outcome_rx: OutcomeReceiver,
            admin_configs: Vec<AdminInterfaceConfig>,
        ) -> ConductorResult<()> {
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

            self.clone().add_admin_interfaces(admin_configs).await?;

            info!("Conductor startup: admin interface(s) added.");

            self.clone().startup_app_interfaces().await?;

            info!("Conductor startup: app interfaces started.");

            // Determine cells to create
            let state = self.get_state().await?;
            let all_enabled_cell_ids = state
                .enabled_apps()
                .flat_map(|(_, app)| app.all_enabled_cells().collect::<Vec<_>>());
            self.create_cells_and_add_to_state(all_enabled_cell_ids)
                .await?;

            info!("Conductor startup: apps enabled.");

            Ok(())
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
            self.ribosome_store().share_ref(|ds| ds.list())
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
        ) -> ConductorResult<IndexMap<CellId, DnaDefHashed>> {
            let mut dna_defs = IndexMap::new();
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
                    let dna_defs: Vec<_> = holochain_state::dna_def::get_all(txn)?
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
                            holochain_state::wasm::get(txn, &wasm_hash)?
                                .map(|hashed| hashed.into_content())
                                .ok_or(ConductorError::WasmMissing)
                                .map(|wasm| (wasm_hash, wasm))
                        })
                        .collect::<ConductorResult<HashMap<_, _>>>()?;
                    let wasms = holochain_state::dna_def::get_all(txn)?
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
                    let defs = holochain_state::entry_def::get_all(txn)?;
                    ConductorResult::Ok((wasms, defs))
                })
                .await?;
            // try to join all the tasks and return the list of dna files
            let wasms = wasms.into_iter().map(|(dna_def, wasms)| async move {
                let dna_file = DnaFile::new(dna_def.into_content(), wasms).await;

                #[cfg(feature = "wasmer_sys")]
                let ribosome =
                    RealRibosome::new(dna_file, self.wasmer_module_cache.clone()).await?;
                #[cfg(feature = "wasmer_wamr")]
                let ribosome = RealRibosome::new(dna_file, None).await?;

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
        pub fn holochain_p2p(&self) -> &holochain_p2p::actor::DynHcP2p {
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
            future::join_all(to_cleanup.into_iter().map(|(cell_id, cell)| async move {
                if let Err(err) = cell.cleanup().await {
                    tracing::error!("Error cleaning up Cell: {:?}\nCellId: {}", err, cell_id);
                }
            }))
            .await;
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
    use super::*;
    use crate::conductor::api::error::{
        zome_call_response_to_conductor_api_result, ConductorApiError,
    };
    use futures::future::join_all;
    use holochain_conductor_api::ZomeCallParamsSigned;
    use holochain_conductor_api::{DnaStorageInfo, StorageBlob, StorageInfo};
    use holochain_sqlite::helpers::BytesSql;
    use holochain_sqlite::sql::sql_peer_meta_store;
    use holochain_sqlite::stats::{get_size_on_disk, get_used_size};
    use holochain_zome_types::block::Block;
    use holochain_zome_types::block::BlockTargetId;
    use kitsune2_api::Url;
    use zome_call_signature_verification::is_valid_signature;

    impl Conductor {
        /// Get signed agent info from the conductor
        pub async fn get_agent_infos(
            &self,
            maybe_dna_hashes: Option<Vec<DnaHash>>,
        ) -> ConductorApiResult<Vec<Arc<AgentInfoSigned>>> {
            let dna_hashes = match maybe_dna_hashes {
                Some(hashes) => hashes,
                None => self
                    .spaces
                    .get_from_spaces(|space| (*space.dna_hash).clone()),
            };

            let mut out = HashSet::new();
            for dna_hash in dna_hashes {
                let peer_store = self
                    .holochain_p2p
                    .peer_store(dna_hash.clone())
                    .await
                    .map_err(|err| ConductorApiError::Other(err.into()))?;
                let all_peers = peer_store.get_all().await?;
                out.extend(all_peers);
            }
            Ok(out.into_iter().collect())
        }

        /// Get signed agent info from the conductor for a given app
        pub async fn get_app_agent_infos(
            &self,
            installed_app_id: &InstalledAppId,
            maybe_dna_hashes: Option<Vec<DnaHash>>,
        ) -> ConductorApiResult<Vec<Arc<AgentInfoSigned>>> {
            let mut app_dnas = self.get_dna_hashes_for_app(installed_app_id).await?;

            if let Some(dna_hashes) = maybe_dna_hashes {
                app_dnas.retain(|h| dna_hashes.contains(h));
            };

            self.get_agent_infos(Some(app_dnas)).await
        }

        /// Get the content of the peer meta store(s) for an agent at a given Url
        /// for spaces (dna hashes) of a specific app
        pub async fn app_peer_meta_info(
            &self,
            installed_app_id: &InstalledAppId,
            url: Url,
            maybe_dna_hashes: Option<Vec<DnaHash>>,
        ) -> ConductorApiResult<BTreeMap<DnaHash, BTreeMap<String, PeerMetaInfo>>> {
            let mut app_hashes = self.get_dna_hashes_for_app(installed_app_id).await?;
            if let Some(dna_hashes) = maybe_dna_hashes {
                app_hashes.retain(|h| dna_hashes.contains(h));
            }
            self.peer_meta_info(url, Some(app_hashes)).await
        }

        /// Get the content of the peer meta store(s) for an agent at a given Url
        pub async fn peer_meta_info(
            &self,
            url: Url,
            maybe_dna_hashes: Option<Vec<DnaHash>>,
        ) -> ConductorApiResult<BTreeMap<DnaHash, BTreeMap<String, PeerMetaInfo>>> {
            let mut space_ids = self
                .spaces
                .get_from_spaces(|space| (*space.dna_hash).clone());

            if let Some(dna_hashes) = maybe_dna_hashes {
                space_ids.retain(|dna_hash| dna_hashes.contains(dna_hash));
            }

            if space_ids.is_empty() {
                return Err(ConductorApiError::Other(
                    "No cell found for the provided dna hashes.".into(),
                ));
            }

            let mut all_infos = BTreeMap::new();

            for dna_hash in space_ids {
                let db = self.spaces.peer_meta_store_db(&dna_hash)?;
                let url2 = url.clone();

                let infos = db
                    .read_async(
                        move |txn| -> DatabaseResult<BTreeMap<String, PeerMetaInfo>> {
                            let mut infos: BTreeMap<String, PeerMetaInfo> = BTreeMap::new();

                            let mut stmt = txn.prepare(sql_peer_meta_store::GET_ALL_BY_URL)?;
                            let mut rows = stmt.query(named_params! {
                                ":peer_url": url2.as_str()
                            })?;

                            while let Some(row) = rows.next()? {
                                let meta_key = row.get::<_, String>(0)?;
                                let meta_value: serde_json::Value =
                                    serde_json::from_slice(&(row.get::<_, BytesSql>(1)?.0))
                                        .map_err(|e| {
                                            rusqlite::Error::FromSqlConversionFailure(
                                                2,
                                                rusqlite::types::Type::Blob,
                                                e.into(),
                                            )
                                        })?;
                                let expires_at = row.get::<_, i64>(2)?;

                                let peer_meta_info = PeerMetaInfo {
                                    meta_value,
                                    expires_at: Timestamp(expires_at),
                                };

                                infos.insert(meta_key, peer_meta_info);
                            }

                            Ok(infos)
                        },
                    )
                    .await?;

                all_infos.insert(dna_hash, infos);
            }

            Ok(all_infos)
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
        ) -> ConductorResult<bool> {
            self.spaces
                .is_blocked(input, timestamp, self.holochain_p2p.clone())
                .await
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn storage_info(&self) -> ConductorResult<StorageInfo> {
            let state = self.get_state().await?;

            let all_dna: HashMap<DnaHash, Vec<InstalledAppId>> = HashMap::new();
            let all_dna =
                state
                    .installed_apps()
                    .iter()
                    .fold(all_dna, |mut acc, (installed_app_id, app)| {
                        for cell_id in app.all_cells() {
                            acc.entry(cell_id.dna_hash().clone())
                                .or_default()
                                .push(installed_app_id.clone());
                        }

                        acc
                    });

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
            used_by: &[InstalledAppId],
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
                dna_hash: dna_hash.clone(),
                used_by: used_by.to_vec(),
            }))
        }

        /// List all host functions provided by this conductor for wasms.
        pub async fn list_wasm_host_functions(&self) -> ConductorApiResult<Vec<String>> {
            Ok(RealRibosome::tooling_imports().await?)
        }

        /// Handle a zome call coming from outside of the conductor, e.g. through the ConductorApi.
        pub async fn handle_external_zome_call(
            &self,
            zome_call_params_signed: ZomeCallParamsSigned,
        ) -> ConductorApiResult<ZomeCallResult> {
            let zome_call_params = zome_call_params_signed
                .bytes
                .clone()
                .decode::<ZomeCallParams>()
                .map_err(|e| ConductorApiError::SerializationError(e.into()))?;
            if !is_valid_signature(
                &zome_call_params.provenance,
                zome_call_params_signed.bytes.as_bytes(),
                &zome_call_params_signed.signature,
            )
            .await?
            {
                return Ok(Ok(ZomeCallResponse::AuthenticationFailed(
                    zome_call_params_signed.signature,
                    zome_call_params.provenance,
                )));
            }

            self.call_zome(zome_call_params.clone()).await
        }

        /// Invoke a zome function on a Cell
        pub async fn call_zome(
            &self,
            params: ZomeCallParams,
        ) -> ConductorApiResult<ZomeCallResult> {
            let cell = self.cell_by_id(&params.cell_id).await?;
            Ok(cell.call_zome(params, None).await?)
        }

        pub(crate) async fn call_zome_with_workspace(
            &self,
            params: ZomeCallParams,
            workspace_lock: SourceChainWorkspace,
        ) -> ConductorApiResult<ZomeCallResult> {
            debug!(cell_id = ?params.cell_id);
            let cell = self.cell_by_id(&params.cell_id).await?;
            Ok(cell.call_zome(params, Some(workspace_lock)).await?)
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
            let call_params = ZomeCallParams {
                cell_id,
                zome_name: zome_name.into(),
                fn_name: fn_name.into(),
                cap_secret,
                provenance: provenance.clone(),
                payload,
                nonce,
                expires_at,
            };
            let response = self.call_zome(call_params).await;
            match response {
                Ok(Ok(response)) => Ok(zome_call_response_to_conductor_api_result(response)?),
                Ok(Err(error)) => Err(ConductorApiError::Other(Box::new(error))),
                Err(error) => Err(error),
            }
        }
    }
}

/// Common install app flags.
#[derive(Default)]
pub struct InstallAppCommonFlags {
    /// From [`AppManifestV0::allow_deferred_memproofs`]
    pub defer_memproofs: bool,
    /// From [`InstallAppPayload::ignore_genesis_failure`]
    pub ignore_genesis_failure: bool,
}

/// Methods related to app installation and management
///
/// Tests related to app installation can be found in ../../tests/tests/app_installation/mod.rs
mod app_impls {
    use holochain_conductor_api::CellInfo;

    use super::*;

    impl Conductor {
        /// Install an app from minimal elements, without needing to construct a whole AppBundle.
        // (This function constructs a bundle under the hood.)
        // This is just a convenience for testing.
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn install_app_minimal(
            self: Arc<Self>,
            installed_app_id: InstalledAppId,
            agent: Option<AgentPubKey>,
            data: &[(impl DnaWithRole, Option<MembraneProof>)],
            network_seed: Option<NetworkSeed>,
        ) -> ConductorResult<AgentPubKey> {
            let dnas_with_roles: Vec<_> = data.iter().map(|(dr, _)| dr).cloned().collect();
            let manifest = app_manifest_from_dnas(&dnas_with_roles, 255, false, network_seed);

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
                .install_app_common(
                    installed_app_id,
                    manifest,
                    agent.clone(),
                    ops,
                    InstallAppCommonFlags {
                        defer_memproofs: false,
                        ignore_genesis_failure: false,
                    },
                )
                .await?;

            Ok(app.agent_key().clone())
        }

        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        async fn install_app_common(
            self: Arc<Self>,
            installed_app_id: InstalledAppId,
            manifest: AppManifest,
            agent_key: Option<AgentPubKey>,
            ops: AppRoleResolution,
            flags: InstallAppCommonFlags,
        ) -> ConductorResult<InstalledApp> {
            let agent_key = match agent_key {
                Some(key) => key,
                None => {
                    // No agent key given. Generate a new key.
                    self.keystore.new_sign_keypair_random().await?
                }
            };

            let cells_to_create = ops.cells_to_create(agent_key.clone());

            // check if cells_to_create contains a cell identical to an existing one
            let state = self.get_state().await?;
            let all_cells: HashSet<_> = state
                .installed_apps()
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

            if flags.defer_memproofs {
                let roles = ops.role_assignments;
                let app = InstalledAppCommon::new(
                    installed_app_id.clone(),
                    agent_key.clone(),
                    roles,
                    manifest,
                    Timestamp::now(),
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

                if genesis_result.is_ok() || flags.ignore_genesis_failure {
                    let roles = ops.role_assignments;
                    let app = InstalledAppCommon::new(
                        installed_app_id.clone(),
                        agent_key.clone(),
                        roles,
                        manifest,
                        Timestamp::now(),
                    )?;

                    // Update the db
                    let disabled_app = self.add_disabled_app_to_db(app).await?;

                    // Return the result, which be may be an error if no_rollback was specified
                    genesis_result.map(|_| disabled_app)
                } else if let Err(err) = genesis_result {
                    Err(err)
                } else {
                    unreachable!()
                }
            }
        }

        /// Install DNAs and set up Cells as specified by an AppBundle
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn install_app_bundle(
            self: Arc<Self>,
            payload: InstallAppPayload,
        ) -> ConductorResult<InstalledApp> {
            let InstallAppPayload {
                source,
                agent_key,
                installed_app_id,
                network_seed,
                roles_settings,
                ignore_genesis_failure,
            } = payload;

            let modifiers = get_modifiers_map_from_role_settings(&roles_settings);
            let membrane_proofs = get_memproof_map_from_role_settings(&roles_settings);
            let existing_cells = get_existing_cells_map_from_role_settings(&roles_settings);

            let bundle = {
                let original_bundle = source.resolve().await?;
                let mut manifest = original_bundle.manifest().to_owned();
                if let Some(network_seed) = network_seed {
                    manifest.set_network_seed(network_seed);
                }
                manifest.override_modifiers(modifiers)?;
                AppBundle::from(original_bundle.into_inner().update_manifest(manifest)?)
            };

            let manifest = bundle.manifest().clone();

            // Use deferred memproofs only if no memproofs are provided for any of the roles.
            // If a memproof is provided for any of the roles, it will override the app wide
            // allow_deferred_memproofs setting and the provided memproofs will be used immediately.
            let defer_memproofs = match &manifest {
                AppManifest::V0(m) => m.allow_deferred_memproofs && membrane_proofs.is_empty(),
            };

            let flags = InstallAppCommonFlags {
                defer_memproofs,
                ignore_genesis_failure,
            };

            let installed_app_id =
                installed_app_id.unwrap_or_else(|| manifest.app_name().to_owned());

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
                .install_app_common(installed_app_id, manifest, agent_key, ops, flags)
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
            let state = self.get_state().await?;
            let deps = state.get_dependent_apps(installed_app_id, true)?;

            // Only uninstall the app if there are no protected dependents,
            // or if force is used
            if force || deps.is_empty() {
                let app = state.get_app(installed_app_id)?;
                let cells_to_remove = app.all_cells().collect::<Vec<_>>();
                // Delete the cells' databases.
                self.delete_cell_databases(app.id(), cells_to_remove.clone())
                    .await?;

                // Delete app from DB and state.
                self.remove_app_from_db(installed_app_id).await?;
                tracing::debug!(msg = "Removed app from db.", app = ?app);

                // Remove the cells from conductor state.
                self.remove_cells(&cells_to_remove).await;

                // Remove the app's signal broadcast from conductor.
                let installed_app_ids = self
                    .get_state()
                    .await?
                    .installed_apps()
                    .iter()
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
        pub async fn list_enabled_apps(&self) -> ConductorResult<Vec<InstalledAppId>> {
            let state = self.get_state().await?;
            Ok(state.enabled_apps().map(|(id, _)| id).cloned().collect())
        }

        /// List Apps with their information,
        /// sorted by their installed_at timestamp, in descending order
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_apps(
            &self,
            status_filter: Option<AppStatusFilter>,
        ) -> ConductorResult<Vec<AppInfo>> {
            use AppStatusFilter::*;
            let conductor_state = self.get_state().await?;

            let apps_ids: Vec<&String> = match status_filter {
                Some(Enabled) => conductor_state.enabled_apps().map(|(id, _)| id).collect(),
                Some(Disabled) => conductor_state.disabled_apps().map(|(id, _)| id).collect(),
                None => conductor_state.installed_apps().keys().collect(),
            };

            let mut app_infos: Vec<AppInfo> = apps_ids
                .into_iter()
                .map(|app_id| self.get_app_info_inner(app_id, &conductor_state))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect();
            app_infos.sort_by_key(|app_info| std::cmp::Reverse(app_info.installed_at));

            Ok(app_infos)
        }

        /// Get the IDs of all active installed Apps which use this Cell
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_enabled_apps_for_dependent_cell_id(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<HashSet<InstalledAppId>> {
            Ok(self
                .get_state()
                .await?
                .enabled_apps()
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
                .enabled_apps()
                .find(|(_, enabled_app)| enabled_app.all_cells().any(|i| i == *cell_id))
                .and_then(|(_, enabled_app)| {
                    enabled_app.role(role_name).ok().map(|role| match role {
                        AppRoleAssignment::Primary(primary) => {
                            CellId::new(primary.dna_hash().clone(), enabled_app.agent_key().clone())
                        }
                        AppRoleAssignment::Dependency(dependency) => dependency.cell_id.clone(),
                    })
                }))
        }

        /// Get the IDs of all active installed Apps which use this Dna
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn list_enabled_apps_for_dependent_dna_hash(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<HashSet<InstalledAppId>> {
            Ok(self
                .get_state()
                .await?
                .enabled_apps()
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

            Ok(())
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

        pub(crate) async fn get_dna_hashes_for_app(
            &self,
            installed_app_id: &InstalledAppId,
        ) -> ConductorResult<Vec<DnaHash>> {
            let app_info = self.get_app_info(installed_app_id).await?.ok_or_else(|| {
                ConductorError::other(format!("App not installed: {}", installed_app_id))
            })?;

            let mut app_dnas: HashSet<DnaHash> = HashSet::new();
            for cell_infos in app_info.cell_info.values() {
                for cell_info in cell_infos {
                    let dna = match cell_info {
                        CellInfo::Provisioned(cell) => cell.cell_id.dna_hash().clone(),
                        CellInfo::Cloned(cell) => cell.cell_id.dna_hash().clone(),
                        CellInfo::Stem(cell) => cell.original_dna_hash.clone(),
                    };
                    app_dnas.insert(dna);
                }
            }

            Ok(app_dnas.into_iter().collect())
        }
    }
}

/// Methods related to cell access
mod cell_impls {
    use super::*;

    impl Conductor {
        pub(crate) async fn cell_by_id(&self, cell_id: &CellId) -> ConductorResult<Arc<Cell>> {
            // Can only get a cell from the running_cells list
            if let Some(cell) = self.running_cells.share_ref(|c| c.get(cell_id).cloned()) {
                Ok(cell)
            } else {
                // If not in running_cells list, check if the cell id is registered at all,
                // to give a different error message for disabled vs missing.
                let present = self
                    .get_state()
                    .await?
                    .installed_apps()
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
        #[cfg(feature = "unstable-migration")]
        pub async fn cells_by_dna_lineage(
            &self,
            dna_hash: &DnaHash,
        ) -> ConductorResult<holochain_conductor_api::CompatibleCells> {
            // TODO: OPTIMIZE: cache the DNA lineages
            use std::collections::BTreeSet;
            Ok(self
                .get_state()
                .await?
                // Look in all installed apps
                .installed_apps()
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
                    "neither network_seed nor properties provided for clone cell".to_string(),
                ));
            }

            let state = self.get_state().await?;
            let app = state.get_app(installed_app_id)?;
            let app_role = app.primary_role(&role_name)?;
            if app_role.is_provisioned {
                // Check source chain if agent key is valid
                let source_chain = SourceChain::new(
                    self.get_or_create_authored_db(app_role.dna_hash(), app.agent_key().clone())?,
                    self.get_or_create_dht_db(app_role.dna_hash())?,
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
            self.create_cells_and_add_to_state([clone_cell.cell_id.clone()].into_iter())
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

            self.create_cells_and_add_to_state([enabled_cell.cell_id.clone()].into_iter())
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
            let (_, cell_id) = self
                .update_state_prime({
                    let app_id = app_id.clone();
                    let clone_cell_id = clone_cell_id.clone();
                    move |mut state| {
                        let app = state.get_app_mut(&app_id)?;
                        let cell_id = app
                            .disabled_clone_cells()
                            .find(|(id, cell_id)| match &clone_cell_id {
                                CloneCellId::CloneId(clone_id) => *id == clone_id,
                                CloneCellId::DnaHash(dna_hash) => cell_id.dna_hash() == dna_hash,
                            })
                            .expect("disabled clone cell not part of this app")
                            .1;
                        let clone_id = app.get_disabled_clone_id(&clone_cell_id)?;
                        app.delete_clone_cell(&clone_id)?;
                        Ok((state, cell_id))
                    }
                })
                .await?;
            self.delete_cell_databases(app_id, vec![cell_id]).await?;
            Ok(())
        }
    }
}

/// Methods related to management of app and cell status
mod app_status_impls {
    use super::*;
    use crate::conductor::cell::error::CellResult;
    use holochain_chc::ChcImpl;

    impl Conductor {
        /// Instantiate cells, join them to the network and add them to the conductor's state.
        pub(crate) async fn create_cells_and_add_to_state(
            self: Arc<Self>,
            cell_ids: impl Iterator<Item = CellId>,
        ) -> ConductorResult<()> {
            let cells_to_create = cell_ids.map(|cell_id| {
                let handle = self.clone();
                async move {
                    handle
                        .clone()
                        .create_cell(&cell_id, handle.get_chc(&cell_id))
                        .await
                }
            });
            // Create cells
            let cells = future::join_all(cells_to_create)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            // Add agents to local agent store in kitsune
            future::join_all(cells.iter().enumerate().map(|(i, (cell, _))| {
                async move {
                    let cell_id = cell.id().clone();
                    let agent_pubkey = cell_id.agent_pubkey().clone();
                    if let Err(e) = cell
                        .holochain_p2p_dna()
                        .clone()
                        .join(agent_pubkey, None)
                        .await
                    {
                        tracing::error!(?e, ?cell_id, "Network join failed.");
                    }
                }
                .instrument(tracing::info_span!("network join task", ?i))
            }))
            .await;

            // Add cells to conductor
            self.add_and_initialize_cells(cells);

            Ok(())
        }

        /// Instantiate a cell.
        async fn create_cell(
            self: Arc<Self>,
            cell_id: &CellId,
            chc: Option<ChcImpl>,
        ) -> CellResult<(Cell, InitialQueueTriggers)> {
            let holochain_p2p_cell = holochain_p2p::HolochainP2pDna::new(
                self.holochain_p2p.clone(),
                cell_id.dna_hash().clone(),
                chc,
            );
            let space = self
                .get_or_create_space(cell_id.dna_hash())
                .map_err(|e| CellError::FailedToCreateDnaSpace(ConductorError::from(e).into()))?;
            let signal_tx = self
                .get_signal_tx(cell_id)
                .await
                .map_err(|err| CellError::ConductorError(Box::new(err)))?;
            tracing::info!(?cell_id, "Creating a cell");
            Cell::create(
                cell_id.clone(),
                self.clone(),
                space,
                holochain_p2p_cell,
                signal_tx,
            )
            .await
        }

        /// Enable an installed app
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
        pub async fn enable_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
        ) -> ConductorResult<InstalledApp> {
            // Check if app can be enabled.
            let state = self.clone().get_state().await?;
            let app = state.get_app(&app_id)?;
            if app.status == AppStatus::AwaitingMemproofs {
                return Err(ConductorError::AppStatusError(
                    "App is awaiting membrane proofs and cannot be enabled.".to_string(),
                ));
            }
            // If app is already enabled, short circuit here.
            if app.status == AppStatus::Enabled {
                return Ok(app.clone());
            }

            // Determine cells to create
            let cell_ids_in_app = app.all_enabled_cells();
            self.clone()
                .create_cells_and_add_to_state(cell_ids_in_app)
                .await?;

            // Set app status to enabled in conductor state.
            let (_, app) = self
                .update_state_prime(move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    app.status = AppStatus::Enabled;
                    let app = app.clone();
                    Ok((state, app))
                })
                .await?;
            Ok(app)
        }

        /// Disable an installed app
        #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
        pub async fn disable_app(
            self: Arc<Self>,
            app_id: InstalledAppId,
            reason: DisabledAppReason,
        ) -> ConductorResult<InstalledApp> {
            let state = self.clone().get_state().await?;
            let app = state.get_app(&app_id)?;

            // If app is already disabled, short circuit here.
            if matches!(app.status, AppStatus::Disabled(_)) {
                return Ok(app.clone());
            }

            // Remove cells from state.
            let cell_ids_to_cleanup = app.all_cells().collect::<Vec<_>>();
            self.remove_cells(&cell_ids_to_cleanup).await;

            // Set app status to disabled.
            let (_, app) = self
                .update_state_prime(move |mut state| {
                    let app = state.get_app_mut(&app_id)?;
                    app.status = AppStatus::Disabled(reason);
                    let app = app.clone();
                    Ok((state, app))
                })
                .await?;
            Ok(app)
        }

        /// Register an app as disabled in the database
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub(crate) async fn add_disabled_app_to_db(
            &self,
            app: InstalledAppCommon,
        ) -> ConductorResult<InstalledApp> {
            let (_, disabled_app) = self
                .update_state_prime(move |mut state| {
                    let disabled_app = state.add_app(app)?;
                    Ok((state, disabled_app))
                })
                .await?;
            Ok(disabled_app)
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
        ///
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
                        db.write_async(|txn| delete_all_ephemeral_scheduled_fns(txn))
                            .await
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
    use super::{state_dump_helpers::peer_store_dump, *};
    use holochain_conductor_api::JsonDump;
    use holochain_zome_types::{action::builder, Entry};
    use kitsune2_api::{SpaceId, TransportStats};
    use std::sync::atomic::Ordering;

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

            source_chain
                .flush(
                    cell.holochain_p2p_dna()
                        .target_arcs()
                        .await
                        .map_err(ConductorApiError::other)?,
                    cell.holochain_p2p_dna().chc(),
                )
                .await?;

            Ok(action_hash)
        }

        /// Revoke a zome call capability for a cell identified by the [`ActionHash`] of the grant.
        pub async fn revoke_zome_call_capability(
            &self,
            cell_id: CellId,
            action_hash: ActionHash,
        ) -> ConductorApiResult<ActionHash> {
            // Must init before committing a grant
            let cell = self.cell_by_id(&cell_id).await?;
            cell.check_or_run_zome_init().await?;

            let source_chain = SourceChain::new(
                self.get_or_create_authored_db(
                    cell_id.dna_hash(),
                    cell.id().agent_pubkey().clone(),
                )?,
                self.get_or_create_dht_db(cell_id.dna_hash())?,
                self.keystore.clone(),
                cell_id.agent_pubkey().clone(),
            )
            .await?;

            // find entry by the action hash
            let grant_query = ChainQueryFilter::new()
                .include_entries(true)
                .entry_type(EntryType::CapGrant);

            let cap_grant_entry = source_chain
                .query(grant_query.clone())
                .await?
                .into_iter()
                .find_map(|record| {
                    if record.action_hash() == &action_hash {
                        match record.entry {
                            RecordEntry::Present(entry) => Some(entry),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .ok_or_else(|| ConductorApiError::other("No cap grant found for action hash"))?;
            let entry_hash = EntryHash::with_data_sync(&cap_grant_entry);

            let action_builder = builder::Delete {
                deletes_address: action_hash,
                deletes_entry_address: entry_hash,
            };
            let action_hash = source_chain
                .put_weightless(action_builder, None, ChainTopOrdering::default())
                .await?;

            source_chain
                .flush(
                    cell.holochain_p2p_dna()
                        .target_arcs()
                        .await
                        .map_err(ConductorApiError::other)?,
                    cell.holochain_p2p_dna().chc(),
                )
                .await?;

            Ok(action_hash)
        }

        /// Get capability grant info for a set of App cells including revoked capabality grants
        pub async fn capability_grant_info(
            &self,
            cell_set: &HashSet<CellId>,
            include_revoked: bool,
        ) -> ConductorApiResult<AppCapGrantInfo> {
            let mut grant_info: Vec<(CellId, Vec<CapGrantInfo>)> = Vec::new();
            let grant_query = ChainQueryFilter::new()
                .include_entries(true)
                .entry_type(EntryType::CapGrant);
            let delete_query: ChainQueryFilter = ChainQueryFilter::new()
                .include_entries(true)
                .action_type(ActionType::Delete);

            for cell_id in cell_set.iter() {
                // create a source chain read to query for the cap grant
                let chain = SourceChainRead::new(
                    self.get_or_create_authored_db(
                        cell_id.dna_hash(),
                        cell_id.agent_pubkey().clone(),
                    )?
                    .into(),
                    self.get_or_create_dht_db(cell_id.dna_hash())?.into(),
                    self.keystore().clone(),
                    cell_id.agent_pubkey().clone(),
                )
                .await?;

                // query for the cap grant and delete actions (capability revokes)
                let grant_list = chain.query(grant_query.clone()).await?;
                // No cap grants for this cell
                if grant_list.is_empty() {
                    continue;
                }
                let delete_action_hash_map: HashMap<ActionHash, Timestamp> = chain
                    .query(delete_query.clone())
                    .await?
                    .iter()
                    .filter_map(|record| {
                        if let Action::Delete(delete) = record.action() {
                            Some((delete.deletes_address.clone(), delete.timestamp))
                        } else {
                            None
                        }
                    })
                    .collect::<HashMap<ActionHash, Timestamp>>();

                tracing::info!("cap grant revocation list: {:?}", delete_action_hash_map);

                // create a list of CapGrantInfo structs for each cell
                let mut cap_grants: Vec<CapGrantInfo> = vec![];
                for grant_record in grant_list {
                    let cap_action_hash = grant_record.action_hash().clone();
                    let mut revoke_time: Option<Timestamp> = None;

                    // skip grant info if include_revoked is false
                    if !include_revoked {
                        continue;
                    // set revoke time if delete action exists
                    } else if delete_action_hash_map.contains_key(&cap_action_hash) {
                        revoke_time = delete_action_hash_map
                            .get(&cap_action_hash)
                            .map(|time| time.to_owned())
                    }
                    let zome_cap_grant = match grant_record.entry.to_grant_option() {
                        Some(zome_cap_grant) => {
                            DesensitizedZomeCallCapGrant::from(zome_cap_grant.clone())
                        }
                        None => continue,
                    };

                    let zome_grant_info = CapGrantInfo {
                        cap_grant: zome_cap_grant,
                        action_hash: cap_action_hash,
                        created_at: grant_record.action().timestamp(),
                        revoked_at: revoke_time,
                    };
                    cap_grants.push(zome_grant_info);
                }
                grant_info.push((cell_id.clone(), cap_grants));
            }
            Ok(AppCapGrantInfo(grant_info))
        }

        /// Create a JSON dump of the cell's state
        #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
        pub async fn dump_cell_state(&self, cell_id: &CellId) -> ConductorApiResult<String> {
            let cell = self.cell_by_id(cell_id).await?;
            let authored_db = cell.get_or_create_authored_db()?;
            let dht_db = cell.dht_db();
            let agent_pub_key = cell_id.agent_pubkey().clone();
            let peer_dump = peer_store_dump(self, cell_id).await?;
            let source_chain_dump =
                source_chain::dump_state(authored_db.clone().into(), agent_pub_key).await?;

            let out = JsonDump {
                peer_dump,
                source_chain_dump,
                integration_dump: integration_dump(dht_db).await?,
            };
            // Add summary
            let summary = out.to_string();
            let out = (out, summary);
            Ok(serde_json::to_string(&out)?)
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
            let peer_dump = peer_store_dump(self, cell_id).await?;
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

        /// Dump of network metrics from Kitsune2.
        pub async fn dump_network_metrics(
            &self,
            request: Kitsune2NetworkMetricsRequest,
        ) -> ConductorApiResult<HashMap<DnaHash, Kitsune2NetworkMetrics>> {
            Ok(self.holochain_p2p.dump_network_metrics(request).await?)
        }

        /// Dump of network metrics from Kitsune2.
        ///
        /// This version of the function filters the metrics to only include connections
        /// relevant to the specified app.
        pub async fn dump_network_metrics_for_app(
            &self,
            installed_app_id: &InstalledAppId,
            request: Kitsune2NetworkMetricsRequest,
        ) -> ConductorApiResult<HashMap<DnaHash, Kitsune2NetworkMetrics>> {
            let all_dna_hashes = {
                let state = self.get_state().await?;
                let installed_app = state.get_app(installed_app_id)?;

                installed_app
                    .role_assignments
                    .values()
                    .flat_map(|r| match r {
                        AppRoleAssignment::Primary(p) if p.is_provisioned => {
                            vec![p.base_dna_hash.clone()]
                        }
                        AppRoleAssignment::Primary(p) => {
                            p.clones.values().cloned().collect::<Vec<_>>()
                        }
                        AppRoleAssignment::Dependency(d) => vec![d.cell_id.dna_hash().clone()],
                    })
                    .collect::<Vec<_>>()
            };

            Ok(if let Some(ref dna_hash) = request.dna_hash {
                if !all_dna_hashes.contains(dna_hash) {
                    return Err(ConductorApiError::Other("DNA hash not found in app".into()));
                }

                self.holochain_p2p.dump_network_metrics(request).await?
            } else {
                let mut out = HashMap::new();
                for dna_hash in all_dna_hashes {
                    match self
                        .holochain_p2p
                        .dump_network_metrics(Kitsune2NetworkMetricsRequest {
                            dna_hash: Some(dna_hash.clone()),
                            ..request.clone()
                        })
                        .await
                    {
                        Ok(metrics) => {
                            out.extend(metrics);
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to get network metrics for dna_hash: {:?}, error: {:?}",
                                dna_hash,
                                e
                            );
                        }
                    }
                }

                out
            })
        }

        /// Dump of backend network stats from the Kitsune2 network transport.
        pub async fn dump_network_stats(&self) -> ConductorApiResult<kitsune2_api::TransportStats> {
            Ok(self.holochain_p2p.dump_network_stats().await?)
        }

        /// Dump of backend network stats from the Kitsune2 network transport.
        ///
        /// This version of the function filters the stats to only include connections
        /// relevant to the specified app.
        pub async fn dump_network_stats_for_app(
            &self,
            installed_app_id: &InstalledAppId,
        ) -> ConductorApiResult<kitsune2_api::TransportStats> {
            let all_dna_hashes = {
                let state = self.get_state().await?;
                let installed_app = state.get_app(installed_app_id)?;

                installed_app
                    .role_assignments
                    .values()
                    .flat_map(|r| match r {
                        AppRoleAssignment::Primary(p) if p.is_provisioned => {
                            vec![p.base_dna_hash.clone()]
                        }
                        AppRoleAssignment::Primary(p) => {
                            p.clones.values().cloned().collect::<Vec<_>>()
                        }
                        AppRoleAssignment::Dependency(d) => vec![d.cell_id.dna_hash().clone()],
                    })
                    .collect::<Vec<_>>()
            };

            let mut keep_peer_ids = HashSet::new();
            for dna_hash in all_dna_hashes {
                let peer_store = self.holochain_p2p.peer_store(dna_hash).await?;
                keep_peer_ids.extend(peer_store.get_all().await?.into_iter().filter_map(|p| {
                    p.url
                        .as_ref()
                        .and_then(|u| u.peer_id())
                        .map(|id| id.to_string())
                }));
            }

            let stats = self.holochain_p2p.dump_network_stats().await?;
            Ok(TransportStats {
                // Common information, fine to return
                backend: stats.backend,
                // These are our peer URLs, always give this back
                peer_urls: stats.peer_urls,
                // This contains connections for the whole conductor, filter it down
                // to only the connections that are relevant to the current app
                connections: stats
                    .connections
                    .into_iter()
                    .filter(|s| keep_peer_ids.contains(&s.pub_key))
                    .collect(),
            })
        }

        /// Add signed agent info to the conductor
        pub async fn add_agent_infos(&self, agent_infos: Vec<String>) -> ConductorApiResult<()> {
            let mut parsed_by_space: HashMap<SpaceId, Vec<Arc<AgentInfoSigned>>> = HashMap::new();
            // Parse agent infos and add them to a map indexed by space id.
            for info in agent_infos {
                let parsed_info = kitsune2_api::AgentInfoSigned::decode(
                    &kitsune2_core::Ed25519Verifier,
                    info.as_bytes(),
                )?;
                let space_id = parsed_info.space.clone();
                parsed_by_space
                    .entry(space_id)
                    .or_default()
                    .push(parsed_info);
            }

            // Add agent infos of a space to the space's peer store.
            for (space_id, agent_infos) in parsed_by_space {
                self.holochain_p2p
                    .peer_store(DnaHash::from_k2_space(&space_id))
                    .await
                    .map_err(|err| ConductorApiError::CellError(err.into()))?
                    .insert(agent_infos)
                    .await?;
            }
            Ok(())
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

/// Methods related to app authentication tokens
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

#[cfg(feature = "unstable-countersigning")]
/// Methods for bridging from host calls to workflows for countersigning
mod countersigning_impls {
    use super::*;
    use crate::core::workflow::{self, countersigning_workflow::CountersigningWorkspace};

    impl Conductor {
        /// Accept a countersigning session
        pub(crate) async fn accept_countersigning_session(
            &self,
            cell_id: CellId,
            request: PreflightRequest,
        ) -> ConductorResult<PreflightRequestAcceptance> {
            let countersigning_trigger = self.cell_by_id(&cell_id).await?.countersigning_trigger();

            Ok(
                workflow::countersigning_workflow::accept_countersigning_request(
                    self.spaces.get_or_create_space(cell_id.dna_hash())?,
                    self.keystore.clone(),
                    cell_id.agent_pubkey().clone(),
                    request,
                    countersigning_trigger,
                )
                .await?,
            )
        }

        /// Get in-memory state of an ongoing countersigning session.
        pub async fn get_countersigning_session_state(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<Option<CountersigningSessionState>> {
            let space = self.get_or_create_space(cell_id.dna_hash())?;
            let maybe_countersigning_workspace =
                space.countersigning_workspaces.lock().get(cell_id).cloned();
            match maybe_countersigning_workspace {
                None => Err(ConductorError::CountersigningError(
                    CountersigningError::WorkspaceDoesNotExist(cell_id.clone()),
                )),
                Some(workspace) => Ok(workspace.get_countersigning_session_state()),
            }
        }

        /// Abandon an ongoing countersigning session when it can not be automatically resolved.
        pub async fn abandon_countersigning_session(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<()> {
            let space = self.get_or_create_space(cell_id.dna_hash())?;
            let countersigning_workspace = self
                .get_workspace_of_unresolved_session(&space, cell_id)
                .await?;
            let cell = self.cell_by_id(cell_id).await?;
            countersigning_workspace.mark_countersigning_session_for_force_abandon(cell_id)?;
            cell.countersigning_trigger()
                .trigger(&"force_abandon_session");
            Ok(())
        }

        /// Publish an ongoing countersigning session when it has not be automatically resolved.
        pub async fn publish_countersigning_session(
            &self,
            cell_id: &CellId,
        ) -> ConductorResult<()> {
            let space = self.get_or_create_space(cell_id.dna_hash())?;
            let countersigning_workspace = self
                .get_workspace_of_unresolved_session(&space, cell_id)
                .await?;
            let cell = self.cell_by_id(cell_id).await?;
            countersigning_workspace.mark_countersigning_session_for_force_publish(cell_id)?;
            cell.countersigning_trigger()
                .trigger(&"force_publish_session");
            Ok(())
        }

        async fn get_workspace_of_unresolved_session(
            &self,
            space: &Space,
            cell_id: &CellId,
        ) -> ConductorResult<Arc<CountersigningWorkspace>> {
            let maybe_countersigning_workspace =
                space.countersigning_workspaces.lock().get(cell_id).cloned();
            match maybe_countersigning_workspace {
                None => Err(ConductorError::CountersigningError(
                    CountersigningError::WorkspaceDoesNotExist(cell_id.clone()),
                )),
                Some(countersigning_workspace) => {
                    match countersigning_workspace.get_countersigning_session_state() {
                        None => Err(ConductorError::CountersigningError(
                            CountersigningError::SessionNotFound(cell_id.clone()),
                        )),
                        Some(CountersigningSessionState::Unknown { resolution, .. }) => {
                            if resolution.attempts >= 1 {
                                Ok(countersigning_workspace)
                            } else {
                                Err(ConductorError::CountersigningError(
                                    CountersigningError::SessionNotUnresolved(cell_id.clone()),
                                ))
                            }
                        }
                        _ => Err(ConductorError::CountersigningError(
                            CountersigningError::SessionNotUnresolved(cell_id.clone()),
                        )),
                    }
                }
            }
        }
    }
}

/// Private methods, only used within the Conductor, never called from outside.
impl Conductor {
    fn add_admin_port(&self, port: u16) {
        self.admin_websocket_ports.share_mut(|p| p.push(port));
    }

    /// Add fully constructed cells to the cell map in the Conductor
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    fn add_and_initialize_cells(&self, cells: Vec<(Cell, InitialQueueTriggers)>) {
        let (new_cells, triggers): (Vec<_>, Vec<_>) = cells.into_iter().unzip();
        self.running_cells.share_mut(|cells| {
            for cell in new_cells {
                let cell_id = cell.id().clone();
                tracing::debug!(?cell_id, "added cell");
                cells.insert(cell_id, Arc::new(cell));
            }
        });
        for trigger in triggers {
            trigger.initialize_workflows();
        }
    }

    async fn delete_or_purge_database<Kind: DbKindT + Send + Sync + 'static>(
        &self,
        db: DbWrite<Kind>,
    ) -> ConductorResult<()> {
        let mut path = db.path().clone();
        if let Err(err) = ffs::remove_file(&path).await {
            tracing::warn!(?err, "Could not remove primary DB file, probably because it is still in use. Purging all data instead.");
            db.write_async(|txn| purge_data(txn)).await?;
        } else {
            tracing::info!("Deleted primary DB file {}", path.display());
        }
        path.set_extension("");
        let stem = path.to_string_lossy();
        for ext in ["shm", "wal"] {
            let path = PathBuf::from(format!("{stem}-{ext}"));
            if let Err(err) = ffs::remove_file(&path).await {
                let err = err.remove_backtrace();
                tracing::warn!(?err, "Failed to remove DB support file");
            } else {
                tracing::info!("Deleted file {}", path.display());
            }
        }
        Ok(())
    }

    /// Delete cell databases.
    ///
    /// All data used by that cell (across Authored, DHT, and Cache databases) will also be deleted.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn delete_cell_databases(
        &self,
        app_id: &InstalledAppId,
        cell_ids: Vec<CellId>,
    ) -> ConductorResult<()> {
        // Delete authored database or purge data
        for cell_id in cell_ids.clone() {
            let authored_db = self
                .spaces
                .get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())?;
            self.delete_or_purge_database(authored_db).await?;
        }

        // Find DNAs of this app which are not used by any other app or agent.
        let remaining_dnas = self
            .get_state()
            .await?
            .installed_apps()
            .iter()
            .filter(|(id, _)| *id != app_id)
            .flat_map(|(_, app)| app.all_cells().map(|cell_id| cell_id.dna_hash().clone()))
            .collect::<Vec<_>>();
        let dnas_to_purge = cell_ids
            .iter()
            .map(|cell_id| cell_id.dna_hash())
            .filter(|dna| !remaining_dnas.contains(dna))
            .collect::<Vec<_>>();

        if !dnas_to_purge.is_empty() {
            tracing::info!(?dnas_to_purge, "Purging DNAs");
        }

        // For any DNAs no longer represented in any installed app,
        // delete DHT and cache databases or purge data.
        for dna_hash in dnas_to_purge {
            // Delete all data from DHT and cache databases.
            // Database files will be deleted after this step, but
            // the DB continues to exist in memory while the conductor
            // is running, supposedly because the pool holds the connection
            // open.
            let dht_db = self.spaces.dht_db(dna_hash)?;
            let cache_db = self.spaces.cache(dna_hash)?;
            futures::future::join_all(
                [
                    dht_db.write_async(|txn| purge_data(txn)).boxed(),
                    cache_db.write_async(|txn| purge_data(txn)).boxed(),
                ]
                .into_iter(),
            )
            .await
            .into_iter()
            .collect::<Result<Vec<()>, _>>()?;

            self.delete_or_purge_database(dht_db).await?;
            self.delete_or_purge_database(cache_db).await?;
        }

        Ok(())
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
                            Box::new(app_role.clone()),
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
                    .installed_apps()
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

    /// Print the current setup in a machine-readable way
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

        pub async fn get_cache_db(
            &self,
            cell_id: &CellId,
        ) -> ConductorApiResult<DbWrite<DbKindCache>> {
            let cell = self.cell_by_id(cell_id).await?;
            Ok(cell.cache().clone())
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
            let chc = conductor.get_chc(&cell_id_inner);
            let ribosome = conductor
                .get_ribosome(cell_id_inner.dna_hash())
                .map_err(Box::new)?;

            Cell::genesis(
                cell_id_inner.clone(),
                conductor,
                authored_db,
                dht_db,
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

/// Get a "standard" AppBundle from a single DNA, with Create provisioning,
/// with no modifiers, and arbitrary role names.
/// Allows setting the clone_limit for every DNA.
pub fn app_manifest_from_dnas(
    dnas_with_roles: &[impl DnaWithRole],
    clone_limit: u32,
    memproofs_deferred: bool,
    network_seed: Option<String>,
) -> AppManifest {
    let roles: Vec<_> = dnas_with_roles
        .iter()
        .map(|dr| {
            let dna = dr.dna();
            let mut modifiers = DnaModifiersOpt::none();
            modifiers.network_seed.clone_from(&network_seed);
            AppRoleManifest {
                name: dr.role(),
                dna: AppRoleDnaManifest {
                    path: Some(format!("{}", dna.dna_hash())),
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
                query_dht_ops_from_statement(txn, state_dump::DHT_OPS_INTEGRATED, dht_ops_cursor)?;

            let validation_limbo = query_dht_ops_from_statement(
                txn,
                state_dump::DHT_OPS_IN_VALIDATION_LIMBO,
                dht_ops_cursor,
            )?;

            let integration_limbo = query_dht_ops_from_statement(
                txn,
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

/// Extract the modifiers from the RoleSettingsMap into their own HashMap
fn get_modifiers_map_from_role_settings(roles_settings: &Option<RoleSettingsMap>) -> ModifiersMap {
    match roles_settings {
        Some(role_settings_map) => role_settings_map
            .iter()
            .filter_map(|(role_name, role_settings)| match role_settings {
                RoleSettings::UseExisting { .. } => None,
                RoleSettings::Provisioned { modifiers, .. } => {
                    modifiers.as_ref().map(|m| (role_name.clone(), m.clone()))
                }
            })
            .collect(),
        None => HashMap::new(),
    }
}

/// Extract the memproofs from the RoleSettingsMap into their own HashMap
fn get_memproof_map_from_role_settings(role_settings: &Option<RoleSettingsMap>) -> MemproofMap {
    match role_settings {
        Some(role_settings_map) => role_settings_map
            .iter()
            .filter_map(|(role_name, role_settings)| match role_settings {
                RoleSettings::UseExisting { .. } => None,
                RoleSettings::Provisioned { membrane_proof, .. } => membrane_proof
                    .as_ref()
                    .map(|m| (role_name.clone(), m.clone())),
            })
            .collect(),
        None => HashMap::new(),
    }
}

/// Extract the existing cells ids from the RoleSettingsMap into their own HashMap
fn get_existing_cells_map_from_role_settings(
    roles_settings: &Option<RoleSettingsMap>,
) -> ExistingCellsMap {
    match roles_settings {
        Some(role_settings_map) => role_settings_map
            .iter()
            .filter_map(|(role_name, role_settings)| match role_settings {
                RoleSettings::UseExisting { cell_id } => Some((role_name.clone(), cell_id.clone())),
                _ => None,
            })
            .collect(),
        None => HashMap::new(),
    }
}
