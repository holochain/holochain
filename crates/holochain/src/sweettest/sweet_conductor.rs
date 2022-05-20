//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::{SweetAgents, SweetApp, SweetAppBatch, SweetCell, SweetConductorHandle};
use crate::conductor::{
    api::error::ConductorApiResult, config::ConductorConfig, error::ConductorResult,
    handle::ConductorHandle, space::Spaces, CellError, Conductor, ConductorBuilder,
};
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_keystore::MetaLairClient;
use holochain_state::prelude::test_db_dir;
use holochain_types::prelude::*;
use holochain_websocket::*;
use kitsune_p2p::KitsuneP2pConfig;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// A stream of signals.
pub type SignalStream = Box<dyn tokio_stream::Stream<Item = Signal> + Send + Sync + Unpin>;

/// A useful Conductor abstraction for testing, allowing startup and shutdown as well
/// as easy installation of apps across multiple Conductors and Agents.
///
/// This is intentionally NOT `Clone`, because the drop handle triggers a shutdown of
/// the conductor handle, which would render all other cloned instances useless.
/// If you need multiple references to a SweetConductor, put it in an Arc
#[derive(derive_more::From)]
pub struct SweetConductor {
    handle: Option<SweetConductorHandle>,
    db_dir: TempDir,
    keystore: MetaLairClient,
    pub(crate) spaces: Spaces,
    config: ConductorConfig,
    dnas: Vec<DnaFile>,
    signal_stream: Option<SignalStream>,
}

/// Standard config for SweetConductors
pub fn standard_config() -> ConductorConfig {
    let mut tuning_params =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    // note, even with this tuning param, the `SSLKEYLOGFILE` env var
    // still must be set in order to enable session keylogging
    tuning_params.danger_tls_keylog = "env_keylog".to_string();
    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning_params);
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    let admin_interface = AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket { port: 0 },
    };
    ConductorConfig {
        network: Some(network),
        admin_interfaces: Some(vec![admin_interface]),
        ..Default::default()
    }
}

impl SweetConductor {
    /// Create a SweetConductor from an already-built ConductorHandle and environments
    /// RibosomeStore
    /// The conductor will be supplied with a single test AppInterface named
    /// "sweet-interface" so that signals may be emitted
    pub async fn new(
        handle: ConductorHandle,
        env_dir: TempDir,
        config: ConductorConfig,
    ) -> SweetConductor {
        // Automatically add a test app interface
        handle
            .add_test_app_interface(Default::default())
            .await
            .expect("Couldn't set up test app interface");

        // Get a stream of all signals since conductor startup
        let signal_stream = handle.signal_broadcaster().await.subscribe_merged();

        // XXX: this is a bit wonky.
        // We create a Spaces instance here purely because it's easier to initialize
        // the per-space databases this way. However, we actually use the TestEnvs
        // to actually access those databases.
        // As a TODO, we can remove the need for TestEnvs in sweettest or have
        // some other better integration between the two.
        let spaces = Spaces::new(env_dir.path().to_path_buf().into(), Default::default()).unwrap();

        let keystore = handle.keystore().clone();

        Self {
            handle: Some(SweetConductorHandle(handle)),
            db_dir: env_dir,
            keystore,
            spaces,
            config,
            dnas: Vec::new(),
            signal_stream: Some(Box::new(signal_stream)),
        }
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_config(config: ConductorConfig) -> SweetConductor {
        let dir = test_db_dir();
        let handle = Self::handle_from_existing(dir.path(), test_keystore(), &config, &[]).await;
        Self::new(handle, dir, config).await
    }

    /// Create a SweetConductor from a partially-configured ConductorBuilder
    pub async fn from_builder(builder: ConductorBuilder) -> SweetConductor {
        let db_dir = test_db_dir();
        let config = builder.config.clone();
        let handle = builder.test(db_dir.path(), &[]).await.unwrap();
        Self::new(handle, db_dir, config).await
    }

    /// Create a handle from an existing environment and config
    pub async fn handle_from_existing(
        db_dir: &Path,
        keystore: MetaLairClient,
        config: &ConductorConfig,
        extra_dnas: &[DnaFile],
    ) -> ConductorHandle {
        Conductor::builder()
            .config(config.clone())
            .with_keystore(keystore)
            .test(db_dir, extra_dnas)
            .await
            .unwrap()
    }

    /// Create a SweetConductor with a new set of TestEnvs from the given config
    pub async fn from_standard_config() -> SweetConductor {
        Self::from_config(standard_config()).await
    }

    /// Access the database path for this conductor
    pub fn db_path(&self) -> &Path {
        self.db_dir.path()
    }

    /// Access the MetaLairClient for this conductor
    pub fn keystore(&self) -> MetaLairClient {
        self.keystore.clone()
    }

    /// Convenience function that uses the internal handle to enable an app
    pub async fn enable_app(
        &self,
        id: InstalledAppId,
    ) -> ConductorResult<(InstalledApp, Vec<(CellId, CellError)>)> {
        self.handle().0.enable_app(id).await
    }

    /// Convenience function that uses the internal handle to disable an app
    pub async fn disable_app(
        &self,
        id: InstalledAppId,
        reason: DisabledAppReason,
    ) -> ConductorResult<InstalledApp> {
        self.handle().0.disable_app(id, reason).await
    }

    /// Convenience function that uses the internal handle to start an app
    pub async fn start_app(&self, id: InstalledAppId) -> ConductorResult<InstalledApp> {
        self.handle().0.start_app(id).await
    }

    /// Convenience function that uses the internal handle to pause an app
    pub async fn pause_app(
        &self,
        id: InstalledAppId,
        reason: PausedAppReason,
    ) -> ConductorResult<InstalledApp> {
        self.handle().0.pause_app(id, reason).await
    }

    /// Install the dna first.
    /// This allows a big speed up when
    /// installing many apps with the same dna
    async fn setup_app_1_register_dna(&mut self, dna_files: &[&DnaFile]) -> ConductorApiResult<()> {
        for &dna_file in dna_files {
            self.register_dna(dna_file.clone()).await?;
            self.dnas.push(dna_file.clone());
        }
        Ok(())
    }

    /// Install the app and enable it
    // TODO: make this take a more flexible config for specifying things like
    // membrane proofs
    async fn setup_app_2_install_and_enable(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[&DnaFile],
    ) -> ConductorApiResult<()> {
        let installed_app_id = installed_app_id.to_string();

        let installed_cells = dna_files
            .iter()
            .map(|&dna| {
                let cell_handle = format!("{}", dna.dna_hash());
                let cell_id = CellId::new(dna.dna_hash().clone(), agent.clone());
                (InstalledCell::new(cell_id, cell_handle), None)
            })
            .collect();
        self.handle()
            .0
            .clone()
            .install_app(installed_app_id.clone(), installed_cells)
            .await?;

        self.handle().0.clone().enable_app(installed_app_id).await?;
        Ok(())
    }

    /// Build the SweetCells after `setup_cells` has been run
    /// The setup is split into two parts because the Cell environments
    /// are not available until after `setup_cells` has run, and it is
    /// better to do that once for all apps in the case of multiple apps being
    /// set up at once.
    async fn setup_app_3_create_sweet_app(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_hashes: impl Iterator<Item = DnaHash>,
    ) -> ConductorApiResult<SweetApp> {
        let mut sweet_cells = Vec::new();
        for dna_hash in dna_hashes {
            // Initialize per-space databases
            let _space = self.spaces.get_or_create_space(&dna_hash)?;

            // Create the SweetCell
            let cell_authored_db = self.handle().0.get_authored_db(&dna_hash)?;
            let cell_dht_db = self.handle().0.get_dht_db(&dna_hash)?;
            let cell_id = CellId::new(dna_hash, agent.clone());
            let cell = SweetCell {
                cell_id,
                cell_authored_db,
                cell_dht_db,
            };
            sweet_cells.push(cell);
        }

        Ok(SweetApp::new(installed_app_id.into(), sweet_cells))
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent<'a, D>(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: D,
    ) -> ConductorApiResult<SweetApp>
    where
        D: IntoIterator<Item = &'a DnaFile>,
    {
        let dna_files: Vec<_> = dna_files.into_iter().collect();
        self.setup_app_1_register_dna(dna_files.as_slice()).await?;
        self.setup_app_2_install_and_enable(installed_app_id, agent.clone(), dna_files.as_slice())
            .await?;

        self.handle()
            .0
            .clone()
            .reconcile_cell_status_with_app_status()
            .await?;

        let dna_files = dna_files.iter().map(|d| d.dna_hash().clone());
        self.setup_app_3_create_sweet_app(installed_app_id, agent, dna_files)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned SweetApp.
    pub async fn setup_app<'a, D>(
        &mut self,
        installed_app_id: &str,
        dna_files: D,
    ) -> ConductorApiResult<SweetApp>
    where
        D: IntoIterator<Item = &'a DnaFile>,
    {
        let agent = SweetAgents::one(self.keystore()).await;
        self.setup_app_for_agent(installed_app_id, agent, dna_files)
            .await
    }

    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    ///
    /// All InstalledAppIds and AppRoleIds are auto-generated. In tests driven directly
    /// by Rust, you typically won't care what these values are set to, but in case you
    /// do, they are set as so:
    /// - InstalledAppId: {app_id_prefix}-{agent_pub_key}
    /// - AppRoleId: {dna_hash}
    ///
    /// Returns a batch of SweetApps, sorted in the same order as Agents passed in.
    pub async fn setup_app_for_agents<'a, A, D>(
        &mut self,
        app_id_prefix: &str,
        agents: A,
        dna_files: D,
    ) -> ConductorApiResult<SweetAppBatch>
    where
        A: IntoIterator<Item = &'a AgentPubKey>,
        D: IntoIterator<Item = &'a DnaFile>,
    {
        let agents: Vec<_> = agents.into_iter().collect();
        let dna_files: Vec<_> = dna_files.into_iter().collect();
        self.setup_app_1_register_dna(dna_files.as_slice()).await?;
        for &agent in agents.iter() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            self.setup_app_2_install_and_enable(
                &installed_app_id,
                agent.to_owned(),
                dna_files.as_slice(),
            )
            .await?;
        }

        self.handle()
            .0
            .clone()
            .reconcile_cell_status_with_app_status()
            .await?;

        let mut apps = Vec::new();
        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            apps.push(
                self.setup_app_3_create_sweet_app(
                    &installed_app_id,
                    agent.clone(),
                    dna_files.iter().map(|d| d.dna_hash().clone()),
                )
                .await?,
            );
        }

        Ok(SweetAppBatch(apps))
    }

    /// Get a stream of all Signals emitted on the "sweet-interface" AppInterface.
    ///
    /// This is designed to crash if called more than once, because as currently
    /// implemented, creating multiple signal streams would simply cause multiple
    /// consumers of the same underlying streams, not a fresh subscription
    pub fn signals(&mut self) -> impl tokio_stream::Stream<Item = Signal> {
        self.signal_stream
            .take()
            .expect("Can't take the SweetConductor signal stream twice")
    }

    /// Get a new websocket client which can send requests over the admin
    /// interface. It presupposes that an admin interface has been configured.
    /// (The standard_config includes an admin interface at port 0.)
    pub async fn admin_ws_client(&self) -> (WebsocketSender, WebsocketReceiver) {
        let port = self
            .get_arbitrary_admin_websocket_port()
            .expect("No admin port open on conductor");
        websocket_client_by_port(port).await.unwrap()
    }

    /// Shutdown this conductor.
    /// This will wait for the conductor to shutdown but
    /// keep the inner state to restart it.
    ///
    /// Attempting to use this conductor without starting it up again will cause a panic.
    pub async fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown_and_wait().await;
        } else {
            panic!("Attempted to shutdown conductor which was already shutdown");
        }
    }

    /// Start up this conductor if it's not already running.
    pub async fn startup(&mut self) {
        if self.handle.is_none() {
            self.handle = Some(SweetConductorHandle(
                Self::handle_from_existing(
                    self.db_dir.path(),
                    self.keystore.clone(),
                    &self.config,
                    self.dnas.as_slice(),
                )
                .await,
            ));
        } else {
            panic!("Attempted to start conductor which was already started");
        }
    }

    /// Check if this conductor is running
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    // NB: keep this private to prevent leaking out owned references
    fn handle(&self) -> SweetConductorHandle {
        self.handle
            .as_ref()
            .map(|h| h.clone_privately())
            .expect("Tried to use a conductor that is offline")
    }

    /// Get the ConductorHandle within this Conductor.
    /// Be careful when using this, because this leaks out handles, which may
    /// make it harder to shut down the conductor during tests.
    pub fn inner_handle(&self) -> ConductorHandle {
        self.handle
            .as_ref()
            .map(|h| h.0.clone())
            .expect("Tried to use a conductor that is offline")
    }

    /// Force trigger all dht ops that haven't received
    /// enough validation receipts yet.
    pub async fn force_all_publish_dht_ops(&self) {
        use futures::stream::StreamExt;
        if let Some(handle) = self.handle.as_ref() {
            let iter = handle.list_cell_ids(None).into_iter().map(|id| async {
                let id = id;
                let db = self.get_authored_db(id.dna_hash()).unwrap();
                let trigger = self.get_cell_triggers(&id).unwrap();
                (db, trigger)
            });
            futures::stream::iter(iter)
                .then(|f| f)
                .for_each(|(db, mut triggers)| async move {
                    // The line below was added when migrating to rust edition 2021, per
                    // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
                    let _ = &triggers;
                    crate::test_utils::force_publish_dht_ops(&db, &mut triggers.publish_dht_ops)
                        .await
                        .unwrap();
                })
                .await;
        }
    }
}

/// Get a websocket client on localhost at the specified port
pub async fn websocket_client_by_port(
    port: u16,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    Ok(holochain_websocket::connect(
        url2::url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?)
}

impl Drop for SweetConductor {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            tokio::task::spawn(async move {
                // Shutdown the conductor
                if let Some(shutdown) = handle.take_shutdown_handle() {
                    handle.shutdown();
                    if let Err(e) = shutdown.await {
                        tracing::warn!("Failed to join conductor shutdown task: {:?}", e);
                    }
                }
            });
        }
    }
}

impl AsRef<SweetConductorHandle> for SweetConductor {
    fn as_ref(&self) -> &SweetConductorHandle {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::ops::Deref for SweetConductor {
    type Target = SweetConductorHandle;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::borrow::Borrow<SweetConductorHandle> for SweetConductor {
    fn borrow(&self) -> &SweetConductorHandle {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}
