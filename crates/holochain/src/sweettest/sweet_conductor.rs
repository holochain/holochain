//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use std::sync::Arc;

use super::{SweetAgents, SweetApp, SweetAppBatch, SweetCell, SweetConductorHandle};
use crate::conductor::{
    config::ConductorConfig, error::ConductorResult, handle::ConductorHandle, Conductor,
    ConductorBuilder,
};
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_conductor_api::{AdminInterfaceConfig, InterfaceDriver};
use holochain_keystore::KeystoreSender;
use holochain_lmdb::test_utils::{test_environments, TestEnvironments};
use holochain_types::prelude::*;
use holochain_websocket::*;
use kitsune_p2p::KitsuneP2pConfig;

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
    envs: TestEnvironments,
    config: ConductorConfig,
    dnas: Vec<DnaFile>,
    signal_stream: Option<SignalStream>,
}

/// Standard config for SweetConductors
pub fn standard_config() -> ConductorConfig {
    let mut network = KitsuneP2pConfig::default();
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
    /// DnaStore
    /// The conductor will be supplied with a single test AppInterface named
    /// "sweet-interface" so that signals may be emitted
    pub async fn new(
        handle: ConductorHandle,
        envs: TestEnvironments,
        config: ConductorConfig,
    ) -> SweetConductor {
        // Automatically add a test app interface
        handle
            .add_test_app_interface(Default::default())
            .await
            .expect("Couldn't set up test app interface");

        // Get a stream of all signals since conductor startup
        let signal_stream = handle.signal_broadcaster().await.subscribe_merged();

        Self {
            handle: Some(SweetConductorHandle(handle)),
            envs,
            config,
            dnas: Vec::new(),
            signal_stream: Some(Box::new(signal_stream)),
        }
    }

    /// Create a SweetConductor with a new set of TestEnvironments from the given config
    pub async fn from_config(config: ConductorConfig) -> SweetConductor {
        let envs = test_environments();
        let handle = Self::handle_from_existing(&envs, &config).await;
        Self::new(handle, envs, config).await
    }

    /// Create a SweetConductor from a partially-configured ConductorBuilder
    pub async fn from_builder<DS: DnaStore + 'static>(
        builder: ConductorBuilder<DS>,
    ) -> SweetConductor {
        let envs = test_environments();
        let config = builder.config.clone();
        let handle = builder.test(&envs).await.unwrap();
        Self::new(handle, envs, config).await
    }

    /// Create a handle from an existing environment and config
    pub async fn handle_from_existing(
        envs: &TestEnvironments,
        config: &ConductorConfig,
    ) -> ConductorHandle {
        Conductor::builder()
            .config(config.clone())
            .test(envs)
            .await
            .unwrap()
    }

    /// Create a SweetConductor with a new set of TestEnvironments from the given config
    pub async fn from_standard_config() -> SweetConductor {
        Self::from_config(standard_config()).await
    }

    /// Access the TestEnvironments for this conductor
    pub fn envs(&self) -> &TestEnvironments {
        &self.envs
    }

    /// Access the KeystoreSender for this conductor
    pub fn keystore(&self) -> KeystoreSender {
        self.envs.keystore()
    }

    /// Install the dna first.
    /// This allows a big speed up when
    /// installing many apps with the same dna
    async fn setup_app_1_register_dna(&mut self, dna_files: &[DnaFile]) -> ConductorResult<()> {
        for dna_file in dna_files {
            self.register_dna(dna_file.clone()).await?;
            self.dnas.push(dna_file.clone());
        }
        Ok(())
    }

    /// Install the app and activate it
    // TODO: make this take a more flexible config for specifying things like
    // membrane proofs
    async fn setup_app_2_install_and_activate(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) -> ConductorResult<()> {
        let installed_app_id = installed_app_id.to_string();

        let installed_cells = dna_files
            .iter()
            .map(|dna| {
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

        self.activate_app(installed_app_id).await?;
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
    ) -> ConductorResult<SweetApp> {
        let mut sweet_cells = Vec::new();
        for dna_hash in dna_hashes {
            let cell_id = CellId::new(dna_hash, agent.clone());
            let cell_env = self.handle().0.get_cell_env(&cell_id).await.unwrap();
            let cell = SweetCell { cell_id, cell_env };
            sweet_cells.push(cell);
        }

        Ok(SweetApp::new(installed_app_id.into(), sweet_cells))
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) -> ConductorResult<SweetApp> {
        self.setup_app_1_register_dna(dna_files).await?;
        self.setup_app_2_install_and_activate(installed_app_id, agent.clone(), dna_files)
            .await?;

        self.handle().0.clone().setup_cells().await?;

        let dna_files = dna_files.iter().map(|d| d.dna_hash().clone());
        self.setup_app_3_create_sweet_app(installed_app_id, agent, dna_files)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned SweetApp.
    pub async fn setup_app(
        &mut self,
        installed_app_id: &str,
        dna_files: &[DnaFile],
    ) -> ConductorResult<SweetApp> {
        let agent = SweetAgents::one(self.keystore()).await;
        self.setup_app_for_agent(installed_app_id, agent, dna_files)
            .await
    }

    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    ///
    /// All InstalledAppIds and CellNicks are auto-generated. In tests driven directly
    /// by Rust, you typically won't care what these values are set to, but in case you
    /// do, they are set as so:
    /// - InstalledAppId: {app_id_prefix}-{agent_pub_key}
    /// - CellNick: {dna_hash}
    ///
    /// Returns a batch of SweetApps, sorted in the same order as Agents passed in.
    pub async fn setup_app_for_agents(
        &mut self,
        app_id_prefix: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> ConductorResult<SweetAppBatch> {
        self.setup_app_1_register_dna(dna_files).await?;
        for agent in agents.iter() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            self.setup_app_2_install_and_activate(&installed_app_id, agent.clone(), dna_files)
                .await?;
        }

        self.handle().0.clone().setup_cells().await?;

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
            .await
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
                Self::handle_from_existing(&self.envs, &self.config).await,
            ));

            // MD: this feels wrong, why should we have to reinstall DNAs on restart?

            for dna_file in self.dnas.iter() {
                self.register_dna(dna_file.clone())
                    .await
                    .expect("Could not install DNA");
            }
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
                if let Some(shutdown) = handle.take_shutdown_handle().await {
                    handle.shutdown().await;
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
