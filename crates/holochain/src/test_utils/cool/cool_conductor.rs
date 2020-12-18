//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::{CoolAgents, CoolApp, CoolAppBatch, CoolCell};
use crate::{
    conductor::{api::ZomeCall, config::ConductorConfig, handle::ConductorHandle, Conductor},
    core::ribosome::ZomeCallInvocation,
};
use futures::future;
use hdk3::prelude::*;
use holochain_keystore::KeystoreSender;
use holochain_state::test_utils::{test_environments, TestEnvironments};
use holochain_types::app::InstalledCell;
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaFile;
use kitsune_p2p::KitsuneP2pConfig;
use std::sync::Arc;
use unwrap_to::unwrap_to;

/// A collection of CoolConductors, with methods for operating on the entire collection
#[derive(
    Clone,
    derive_more::AsRef,
    derive_more::From,
    derive_more::Into,
    derive_more::IntoIterator,
    derive_more::AsMut,
)]
pub struct CoolConductorBatch(Vec<CoolConductor>);

impl CoolConductorBatch {
    /// Map the given ConductorConfigs into CoolConductors, each with its own new TestEnvironments
    pub async fn from_configs<I: IntoIterator<Item = ConductorConfig>>(
        configs: I,
    ) -> CoolConductorBatch {
        future::join_all(configs.into_iter().map(CoolConductor::from_config))
            .await
            .into()
    }

    /// Create the given number of new CoolConductors, each with its own new TestEnvironments
    pub async fn from_config(num: usize, config: ConductorConfig) -> CoolConductorBatch {
        Self::from_configs(std::iter::repeat(config).take(num)).await
    }

    /// Create the given number of new CoolConductors, each with its own new TestEnvironments
    pub async fn from_standard_config(num: usize) -> CoolConductorBatch {
        Self::from_configs(std::iter::repeat_with(standard_config).take(num)).await
    }

    /// Get the underlying data
    pub fn iter(&self) -> impl Iterator<Item = &CoolConductor> {
        self.0.iter()
    }

    /// Get the underlying data
    pub fn into_inner(self) -> Vec<CoolConductor> {
        self.0
    }

    /// Opinionated app setup.
    /// Creates one app on each Conductor in this batch, creating a new AgentPubKey for each.
    /// The created AgentPubKeys can be retrieved via each CoolApp.
    pub async fn setup_app(&self, installed_app_id: &str, dna_files: &[DnaFile]) -> CoolAppBatch {
        let apps = self
            .0
            .iter()
            .map(|conductor| async move {
                let agent = CoolAgents::one(conductor.keystore()).await;
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
    /// Returns a batch of CoolApps, sorted in the same order as the Conductors in
    /// this batch.
    pub async fn setup_app_for_zipped_agents(
        &self,
        installed_app_id: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> CoolAppBatch {
        if agents.len() != self.0.len() {
            panic!("setup_app_for_zipped_agents must take as many Agents as there are Conductors in this batch.")
        }

        let apps = self
            .0
            .iter()
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

// #[derive(Clone, shrinkwraprs::Shrinkwrap, derive_more::From)]
#[derive(Clone, derive_more::From)]
/// A wrapper around ConductorHandle with more convenient methods for testing
pub struct CoolConductor {
    // #[shrinkwrap(main_field)]
    handle: Option<Arc<CoolConductorInner>>,
    envs: TestEnvironments,
    config: ConductorConfig,
    dnas: Arc<std::sync::Mutex<Vec<DnaFile>>>,
}

/// Inner handle with a cleanup drop
#[derive(shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct CoolConductorInner(pub(crate) ConductorHandle);

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

impl CoolConductor {
    /// Create a CoolConductor from an already-build ConductorHandle and environments
    pub fn new(
        handle: ConductorHandle,
        envs: TestEnvironments,
        config: ConductorConfig,
    ) -> CoolConductor {
        let handle = Arc::new(CoolConductorInner(handle));
        Self {
            handle: Some(handle),
            envs,
            config,
            dnas: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Create a CoolConductor with a new set of TestEnvironments from the given config
    pub async fn from_config(config: ConductorConfig) -> CoolConductor {
        let envs = test_environments();
        let handle = Self::from_existing(&envs, &config).await;
        Self::new(handle, envs, config)
    }

    /// Create a handle from an existing environment and config
    async fn from_existing(envs: &TestEnvironments, config: &ConductorConfig) -> ConductorHandle {
        Conductor::builder()
            .config(config.clone())
            .test(envs)
            .await
            .unwrap()
    }

    /// Create a CoolConductor with a new set of TestEnvironments from the given config
    pub async fn from_standard_config() -> CoolConductor {
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

    /// TODO: make this take a more flexible config for specifying things like
    /// membrane proofs
    async fn setup_app_inner(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) -> CoolApp {
        let installed_app_id = installed_app_id.to_string();

        for dna_file in dna_files {
            self.install_dna(dna_file.clone())
                .await
                .expect("Could not install DNA");
            self.dnas.lock().unwrap().push(dna_file.clone());
        }

        let cool_cells: Vec<CoolCell> = dna_files
            .iter()
            .map(|dna| CellId::new(dna.dna_hash().clone(), agent.clone()))
            .map(|cell_id| CoolCell {
                cell_id,
                handle: self.clone(),
            })
            .collect();
        let installed_cells = cool_cells
            .iter()
            .map(|cell| {
                (
                    InstalledCell::new(
                        cell.cell_id().clone(),
                        format!("{}", cell.cell_id().dna_hash()),
                    ),
                    None,
                )
            })
            .collect();
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
            .0
            .clone()
            .install_app(installed_app_id.clone(), installed_cells)
            .await
            .expect("Could not install app");
        CoolApp::new(installed_app_id, cool_cells)
    }

    /// Opinionated app setup.
    /// Creates an app for the given agent, using the given DnaFiles, with no extra configuration.
    pub async fn setup_app_for_agent(
        &self,
        installed_app_id: &str,
        agent: AgentPubKey,
        dna_files: &[DnaFile],
    ) -> CoolApp {
        let app = self
            .setup_app_inner(installed_app_id, agent, dna_files)
            .await;

        self.activate_app(app.installed_app_id().clone())
            .await
            .expect("Could not activate app");

        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
            .0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        app
    }

    /// Opinionated app setup.
    /// Creates an app using the given DnaFiles, with no extra configuration.
    /// An AgentPubKey will be generated, and is accessible via the returned CoolApp.
    pub async fn setup_app(&self, installed_app_id: &str, dna_files: &[DnaFile]) -> CoolApp {
        let agent = CoolAgents::one(self.keystore()).await;
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
    /// Returns a batch of CoolApps, sorted in the same order as Agents passed in.
    pub async fn setup_app_for_agents(
        &self,
        app_id_prefix: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> CoolAppBatch {
        let mut apps = Vec::new();
        for agent in agents.to_vec() {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            let app = self
                .setup_app_inner(&installed_app_id, agent, dna_files)
                .await;
            apps.push(app);
        }

        for app in apps.iter() {
            self.activate_app(app.installed_app_id().clone())
                .await
                .expect("Could not activate app");
        }

        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
            .0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        CoolAppBatch(apps)
    }

    /// Shutdown this conductor.
    /// This will wait for the conductor to shutdown but
    /// keep the inner state to restart it.
    ///
    /// Using this conductor without starting it up again will panic.
    pub async fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.wait_for_shutdown().await;
        }
    }

    /// Start up this conductor if it's not already running.
    pub async fn startup(&mut self) {
        if self.handle.is_none() {
            self.handle = Some(Arc::new(CoolConductorInner(
                Self::from_existing(&self.envs, &self.config).await,
            )));
            for dna_file in self.dnas.lock().unwrap().iter() {
                self.install_dna(dna_file.clone())
                    .await
                    .expect("Could not install DNA");
            }
        }
    }

    /// Check if this conductor is running
    pub async fn is_running(&self) -> bool {
        self.handle.is_some()
    }
}

impl CoolConductorInner {
    /// Call a zome function with automatic de/serialization of input and output
    /// and unwrapping of nested errors.
    pub async fn call_zome_ok<'a, I, O, F, E>(&'a self, invocation: CoolZomeCall<'a, I, F, E>) -> O
    where
        E: std::fmt::Debug,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        let response = self.0.call_zome(invocation.into()).await.unwrap().unwrap();
        unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .expect("Couldn't deserialize zome call output")
    }

    /// `call_zome_ok`, but with arguments provided individually
    pub async fn call_zome_ok_flat<I, O, Z, F, E>(
        &self,
        cell_id: &CellId,
        zome_name: Z,
        fn_name: F,
        cap: Option<CapSecret>,
        provenance: Option<AgentPubKey>,
        payload: I,
    ) -> O
    where
        E: std::fmt::Debug,
        ZomeName: From<Z>,
        FunctionName: From<F>,
        SerializedBytes: TryFrom<I, Error = E>,
        O: TryFrom<SerializedBytes, Error = E> + std::fmt::Debug,
    {
        let payload = ExternInput::new(payload.try_into().expect("Couldn't serialize payload"));
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        let call = ZomeCall {
            cell_id: cell_id.clone(),
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cap,
            provenance,
            payload,
        };
        let response = self.0.call_zome(call).await.unwrap().unwrap();
        unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()
            .expect("Couldn't deserialize zome call output")
    }

    /// Manually await shutting down the conductor.
    /// Conductors are already cleaned up on drop but this
    /// is useful if you need to know when it's finished cleaning up.
    pub async fn wait_for_shutdown(&self) {
        let c = &self.0;
        if let Some(shutdown) = c.take_shutdown_handle().await {
            c.shutdown().await;
            shutdown.await.expect("Failed to await shutdown handle");
        }
    }
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[derive(Clone, Debug)]
pub struct CoolZomeCall<'a, P, F, E>
where
    SerializedBytes: TryFrom<P, Error = E>,
    E: std::fmt::Debug,
    FunctionName: From<F>,
{
    /// The Id of the `Cell` in which this Zome-call would be invoked
    pub cell_id: &'a CellId,
    /// The Zome containing the function that would be invoked
    pub zome: &'a Zome,
    /// The capability request authorization.
    /// This can be `None` and still succeed in the case where the function
    /// in the zome being called has been given an Unrestricted status
    /// via a `CapGrant`. Otherwise, it will be necessary to provide a `CapSecret` for every call.
    pub cap: Option<CapSecret>,
    /// The name of the Zome function to call
    pub fn_name: F,
    /// The data to be serialized and passed as an argument to the Zome call
    pub payload: P,
    /// If None, the AgentPubKey from the CellId is used (a common case)
    pub provenance: Option<AgentPubKey>,
}

impl<'a, P, F, E> From<CoolZomeCall<'a, P, F, E>> for ZomeCallInvocation
where
    SerializedBytes: TryFrom<P, Error = E>,
    E: std::fmt::Debug,
    FunctionName: From<F>,
{
    fn from(czc: CoolZomeCall<'a, P, F, E>) -> Self {
        let CoolZomeCall {
            cell_id,
            zome,
            fn_name,
            cap,
            provenance,
            payload,
        } = czc;
        let payload = ExternInput::new(payload.try_into().expect("Couldn't serialize payload"));
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        ZomeCallInvocation {
            cell_id: cell_id.clone(),
            zome: zome.clone(),
            fn_name: fn_name.into(),
            cap,
            provenance,
            payload,
        }
    }
}

impl<'a, P, F, E> From<CoolZomeCall<'a, P, F, E>> for ZomeCall
where
    SerializedBytes: TryFrom<P, Error = E>,
    E: std::fmt::Debug,
    FunctionName: From<F>,
{
    fn from(czc: CoolZomeCall<'a, P, F, E>) -> Self {
        ZomeCallInvocation::from(czc).into()
    }
}

impl Drop for CoolConductorInner {
    fn drop(&mut self) {
        let c = self.0.clone();
        tokio::task::spawn(async move {
            // Shutdown the conductor
            if let Some(shutdown) = c.take_shutdown_handle().await {
                c.shutdown().await;
                shutdown.await.expect("Failed to await shutdown handle");
            }
        });
    }
}

// impl From<ConductorHandle> for CoolConductor {
//     fn from(h: ConductorHandle) -> Self {
//         CoolConductor(Arc::new(CoolConductorInner(h)))
//     }
// }

impl AsRef<Arc<CoolConductorInner>> for CoolConductor {
    fn as_ref(&self) -> &Arc<CoolConductorInner> {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}
impl std::ops::Deref for CoolConductor {
    type Target = Arc<CoolConductorInner>;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}
impl std::borrow::Borrow<Arc<CoolConductorInner>> for CoolConductor {
    fn borrow(&self) -> &Arc<CoolConductorInner> {
        self.handle
            .as_ref()
            .expect("Tried to use a conductor that is offline")
    }
}

impl std::ops::Index<usize> for CoolConductorBatch {
    type Output = CoolConductor;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl std::ops::IndexMut<usize> for CoolConductorBatch {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
