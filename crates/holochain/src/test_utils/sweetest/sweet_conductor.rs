//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::{SweetAgents, SweetApp, SweetAppBatch, SweetCell, SweetZome};
use crate::conductor::{
    api::{error::ConductorApiResult, ZomeCall},
    config::ConductorConfig,
    handle::ConductorHandle,
    Conductor, ConductorBuilder,
};
use futures::future;
use hdk::prelude::*;
use holo_hash::DnaHash;
use holochain_keystore::KeystoreSender;
use holochain_lmdb::test_utils::{test_environments, TestEnvironments};
use holochain_types::prelude::*;
use kitsune_p2p::KitsuneP2pConfig;
use std::sync::Arc;
use unwrap_to::unwrap_to;

/// A collection of SweetConductors, with methods for operating on the entire collection
#[derive(derive_more::From, derive_more::Into, derive_more::IntoIterator)]
pub struct SweetConductorBatch(Vec<SweetConductor>);

impl SweetConductorBatch {
    /// Map the given ConductorConfigs into SweetConductors, each with its own new TestEnvironments
    pub async fn from_configs<I: IntoIterator<Item = ConductorConfig>>(
        configs: I,
    ) -> SweetConductorBatch {
        future::join_all(configs.into_iter().map(SweetConductor::from_config))
            .await
            .into()
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_config(num: usize, config: ConductorConfig) -> SweetConductorBatch {
        Self::from_configs(std::iter::repeat(config).take(num)).await
    }

    /// Create the given number of new SweetConductors, each with its own new TestEnvironments
    pub async fn from_standard_config(num: usize) -> SweetConductorBatch {
        Self::from_configs(std::iter::repeat_with(standard_config).take(num)).await
    }

    /// Get the underlying data
    pub fn iter(&self) -> impl Iterator<Item = &SweetConductor> {
        self.0.iter()
    }

    /// Get the underlying data
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SweetConductor> {
        self.0.iter_mut()
    }

    /// Get the underlying data
    pub fn into_inner(self) -> Vec<SweetConductor> {
        self.0
    }

    /// Opinionated app setup.
    /// Creates one app on each Conductor in this batch, creating a new AgentPubKey for each.
    /// The created AgentPubKeys can be retrieved via each SweetApp.
    pub async fn setup_app(
        &mut self,
        installed_app_id: &str,
        dna_files: &[DnaFile],
    ) -> SweetAppBatch {
        let apps = self
            .0
            .iter_mut()
            .map(|conductor| async move {
                let agent = SweetAgents::one(conductor.keystore()).await;
                conductor
                    .setup_app_for_agent(installed_app_id, agent, dna_files)
                    .await
            })
            .collect::<Vec<_>>();

        future::join_all(apps).await.into()
    }

    /// Opinionated app setup. Creates one app on each Conductor in this batch,
    /// using the given agents and DnaFiles.
    ///
    /// The number of Agents passed in must be the same as the number of Conductors
    /// in this batch. Each Agent will be used to create one app on one Conductor,
    /// hence the "zipped" in the function name
    ///
    /// Returns a batch of SweetApps, sorted in the same order as the Conductors in
    /// this batch.
    pub async fn setup_app_for_zipped_agents(
        &mut self,
        installed_app_id: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> SweetAppBatch {
        if agents.len() != self.0.len() {
            panic!(
                "setup_app_for_zipped_agents must take as many Agents as there are Conductors in this batch."
            )
        }

        let apps = self
            .0
            .iter_mut()
            .zip(agents.iter())
            .map(|(conductor, agent)| {
                conductor.setup_app_for_agent(installed_app_id, agent.clone(), dna_files)
            })
            .collect::<Vec<_>>();

        future::join_all(apps).await.into()
    }

    /// Let each conductor know about each others' agents so they can do networking
    pub async fn exchange_peer_info(&self) {
        let envs = self.0.iter().map(|c| c.envs().p2p()).collect();
        crate::conductor::p2p_store::exchange_peer_info(envs);
    }
}

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
    handle: Option<Arc<SweetConductorHandle>>,
    envs: TestEnvironments,
    config: ConductorConfig,
    dnas: Vec<DnaFile>,
    signal_stream: Option<SignalStream>,
}

fn standard_config() -> ConductorConfig {
    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    ConductorConfig {
        network: Some(network),
        ..Default::default()
    }
}

impl SweetConductor {
    /// Create a SweetConductor from an already-built ConductorHandle and environments
    ///DnaStore
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
            handle: Some(Arc::new(SweetConductorHandle(handle))),
            envs,
            config,
            dnas: Vec::new(),
            signal_stream: Some(Box::new(signal_stream)),
        }
    }

    /// Create a SweetConductor with a new set of TestEnvironments from the given config
    pub async fn from_config(config: ConductorConfig) -> SweetConductor {
        let envs = test_environments();
        let handle = Self::from_existing(&envs, &config).await;
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
    async fn from_existing(envs: &TestEnvironments, config: &ConductorConfig) -> ConductorHandle {
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
    async fn setup_app_part_1(&mut self, dna_files: &[DnaFile]) {
        for dna_file in dna_files {
            self.register_dna(dna_file.clone())
                .await
                .expect("Could not install DNA");
            self.dnas.push(dna_file.clone());
        }
    }

    /// Install the app and activate it
    // TODO: make this take a more flexible config for specifying things like
    // membrane proofs
    async fn setup_app_part_2(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) {
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
            .await
            .expect("Could not install app");

        self.activate_app(installed_app_id)
            .await
            .expect("Could not activate app");
    }

    /// Build the SweetCells after `setup_cells` has been run
    /// The setup is split into two parts because the Cell environments
    /// are not available until after `setup_cells` has run, and it is
    /// better to do that once for all apps in the case of multiple apps being
    /// set up at once.
    async fn setup_app_part_3(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_hashes: impl Iterator<Item = DnaHash>,
    ) -> SweetApp {
        let mut sweet_cells = Vec::new();
        for dna_hash in dna_hashes {
            let cell_id = CellId::new(dna_hash, agent.clone());
            let cell_env = self
                .handle()
                .0
                .get_cell_env(&cell_id)
                .await
                .expect("Couldn't get cell environment");
            let cell = SweetCell { cell_id, cell_env };
            sweet_cells.push(cell);
        }

        SweetApp::new(installed_app_id.into(), sweet_cells)
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent(
        &mut self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) -> SweetApp {
        self.setup_app_part_1(dna_files).await;
        self.setup_app_part_2(installed_app_id, agent.clone(), dna_files)
            .await;

        self.handle()
            .0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        let dna_files = dna_files.iter().map(|d| d.dna_hash().clone());
        self.setup_app_part_3(installed_app_id, agent, dna_files)
            .await
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned SweetApp.
    pub async fn setup_app(&mut self, installed_app_id: &str, dna_files: &[DnaFile]) -> SweetApp {
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
    ) -> SweetAppBatch {
        self.setup_app_part_1(dna_files).await;
        for agent in agents.iter() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            self.setup_app_part_2(&installed_app_id, agent.clone(), dna_files)
                .await;
        }

        self.handle()
            .0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        let mut apps = Vec::new();
        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            apps.push(
                self.setup_app_part_3(
                    &installed_app_id,
                    agent.clone(),
                    dna_files.iter().map(|d| d.dna_hash().clone()),
                )
                .await,
            );
        }

        SweetAppBatch(apps)
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
            self.handle = Some(Arc::new(SweetConductorHandle(
                Self::from_existing(&self.envs, &self.config).await,
            )));

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
    fn handle(&self) -> Arc<SweetConductorHandle> {
        self.handle
            .clone()
            .expect("Tried to use a conductor that is offline")
    }
}
/// A wrapper around ConductorHandle with more convenient methods for testing
/// and a cleanup drop
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct SweetConductorHandle(pub(crate) ConductorHandle);

impl SweetConductorHandle {
    /// Make a zome call to a Cell, as if that Cell were the caller. Most common case.
    /// No capability is necessary, since the authorship capability is automatically granted.
    pub async fn call<I, O, F>(&self, zome: &SweetZome, fn_name: F, payload: I) -> O
    where
        FunctionName: From<F>,
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_fallible(zome, fn_name, payload).await.unwrap()
    }

    /// Like `call`, but without the unwrap
    pub async fn call_fallible<I, O, F>(
        &self,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        FunctionName: From<F>,
        I: serde::Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(zome.cell_id().agent_pubkey(), None, zome, fn_name, payload)
            .await
    }

    /// Make a zome call to a Cell, as if some other Cell were the caller. More general case.
    /// Can optionally provide a capability.
    pub async fn call_from<I, O, F>(
        &self,
        provenance: &AgentPubKey,
        cap: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> O
    where
        FunctionName: From<F>,
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        self.call_from_fallible(provenance, cap, zome, fn_name, payload)
            .await
            .unwrap()
    }

    /// Like `call_from`, but without the unwrap
    pub async fn call_from_fallible<I, O, F>(
        &self,
        provenance: &AgentPubKey,
        cap: Option<CapSecret>,
        zome: &SweetZome,
        fn_name: F,
        payload: I,
    ) -> ConductorApiResult<O>
    where
        FunctionName: From<F>,
        I: Serialize + std::fmt::Debug,
        O: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let payload = ExternIO::encode(payload).expect("Couldn't serialize payload");
        let call = ZomeCall {
            cell_id: zome.cell_id().clone(),
            zome_name: zome.name().clone(),
            fn_name: fn_name.into(),
            cap,
            provenance: provenance.clone(),
            payload,
        };
        self.0.call_zome(call).await.map(|r| {
            unwrap_to!(r.unwrap() => ZomeCallResponse::Ok)
                .decode()
                .expect("Couldn't deserialize zome call output")
        })
    }

    // /// Get a stream of all Signals emitted since the time of this function call.
    // pub async fn signal_stream(&self) -> impl tokio_stream::Stream<Item = Signal> {
    //     self.0.signal_broadcaster().await.subscribe_merged()
    // }

    /// Manually await shutting down the conductor.
    /// Conductors are already cleaned up on drop but this
    /// is useful if you need to know when it's finished cleaning up.
    pub async fn shutdown_and_wait(&self) {
        let c = &self.0;
        if let Some(shutdown) = c.take_task_manager().await {
            c.shutdown().await;
            shutdown
                .await
                .expect("Failed to await shutdown handle")
                .expect("Conductor shutdown error");
        }
    }
}

impl Drop for SweetConductor {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            tokio::task::spawn(async move {
                // Shutdown the conductor
                if let Some(shutdown) = handle.take_task_manager().await {
                    handle.shutdown().await;
                    if let Err(e) = shutdown.await {
                        tracing::warn!("Failed to join conductor shutdown task: {:?}", e);
                    }
                }
            });
        }
    }
}

impl AsRef<Arc<SweetConductorHandle>> for SweetConductor {
    fn as_ref(&self) -> &Arc<SweetConductorHandle> {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::ops::Deref for SweetConductor {
    type Target = Arc<SweetConductorHandle>;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::borrow::Borrow<Arc<SweetConductorHandle>> for SweetConductor {
    fn borrow(&self) -> &Arc<SweetConductorHandle> {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::ops::Index<usize> for SweetConductorBatch {
    type Output = SweetConductor;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for SweetConductorBatch {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
