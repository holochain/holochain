//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::CoolCell;
use crate::{
    conductor::{api::ZomeCall, config::ConductorConfig, handle::ConductorHandle, Conductor},
    core::ribosome::ZomeCallInvocation,
};
use futures::future;
use hdk3::prelude::*;
use holochain_keystore::KeystoreSender;
use holochain_state::test_utils::{test_environments, TestEnvironments};
use holochain_types::app::{InstalledAppId, InstalledCell};
use holochain_types::dna::zome::Zome;
use holochain_types::dna::DnaFile;
use itertools::traits::HomogeneousTuple;
use kitsune_p2p::KitsuneP2pConfig;
use std::sync::Arc;
use unwrap_to::unwrap_to;

#[derive(Clone, shrinkwraprs::Shrinkwrap, derive_more::From)]
/// A wrapper around ConductorHandle with more convenient methods for testing
pub struct CoolConductor {
    #[shrinkwrap(main_field)]
    handle: Arc<CoolConductorInner>,
    envs: TestEnvironments,
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
    pub fn new(handle: ConductorHandle, envs: TestEnvironments) -> CoolConductor {
        let handle = Arc::new(CoolConductorInner(handle));
        Self { handle, envs }
    }

    /// Create a CoolConductor with a new set of TestEnvironments from the given config
    pub async fn from_config(config: ConductorConfig) -> CoolConductor {
        let envs = test_environments();
        let handle = Conductor::builder()
            .config(config)
            .test(&envs)
            .await
            .unwrap()
            .into();
        Self::new(handle, envs)
    }

    /// Create a CoolConductor with a new set of TestEnvironments from the given config
    pub async fn from_standard_config() -> CoolConductor {
        Self::from_config(standard_config()).await
    }

    /// Map the given ConductorConfigs into CoolConductors, each with its own new TestEnvironments
    pub async fn multi_from_configs<I: IntoIterator<Item = ConductorConfig>>(
        configs: I,
    ) -> Vec<CoolConductor> {
        future::join_all(configs.into_iter().map(Self::from_config)).await
    }

    /// Create the given number of new CoolConductors, each with its own new TestEnvironments
    pub async fn multi_from_config(num: usize, config: ConductorConfig) -> Vec<CoolConductor> {
        Self::multi_from_configs(std::iter::repeat(config).take(num)).await
    }

    /// Create the given number of new CoolConductors, each with its own new TestEnvironments
    pub async fn multi_from_standard_config(num: usize) -> Vec<CoolConductor> {
        Self::multi_from_configs(std::iter::repeat_with(standard_config).take(num)).await
    }

    /// Access the TestEnvironments for this conductor
    pub fn envs(&self) -> &TestEnvironments {
        &self.envs
    }

    /// Access the KeystoreSender for this conductor
    pub fn keystore(&self) -> KeystoreSender {
        self.envs.keystore()
    }

    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    ///
    /// All InstalledAppIds and CellNicks are auto-generated. In tests driven directly
    /// by Rust, you typically won't care what these values are set to, but in case you
    /// do, they are set as so:
    /// - InstalledAppId: {app_id_prefix}-{agent_pub_key}
    /// - CellNick: {dna_hash}
    ///
    /// Returns the list of generated InstalledAppIds, in the same order as Agents passed in.
    pub async fn setup_app_for_agents_with_no_membrane_proof(
        &self,
        app_id_prefix: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> CoolInstalledApps {
        for dna_file in dna_files {
            self.handle
                .install_dna(dna_file.clone())
                .await
                .expect("Could not install DNA")
        }

        let mut info = Vec::new();

        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            let cell_ids: Vec<CoolCell> = dna_files
                .iter()
                .map(|f| CellId::new(f.dna_hash().clone(), agent.clone()))
                .map(|cell_id| CoolCell {
                    cell_id,
                    handle: self.clone(),
                })
                .collect();
            let cells = cell_ids
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
                .0
                .clone()
                .install_app(installed_app_id.clone(), cells)
                .await
                .expect("Could not install app");
            info.push((installed_app_id, cell_ids));
        }

        for (installed_app_id, _) in info.iter() {
            self.handle
                .activate_app(installed_app_id.clone())
                .await
                .expect("Could not activate app");
        }

        self.handle
            .0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        CoolInstalledApps(info)
    }
}

/// Return type of opinionated setup function
pub struct CoolInstalledApps(CoolInstalledAppsRaw);

type CoolInstalledAppsRaw = Vec<(InstalledAppId, Vec<CoolCell>)>;

impl CoolInstalledApps {
    /// Get the underlying data
    pub fn into_inner(self) -> CoolInstalledAppsRaw {
        self.0
    }

    /// Helper to destructure the nested app setup return value as nested tuples.
    /// Each level of nesting can contain 1-4 items, i.e. up to 4 agents with 4 DNAs each.
    /// Beyond 4, and this will PANIC! (But it's just for tests so it's fine.)
    pub fn into_tuples<Outer, Inner>(self) -> Outer
    where
        Outer: HomogeneousTuple<Item = Inner>,
        Inner: HomogeneousTuple<Item = CoolCell>,
        Outer::Buffer: std::convert::AsRef<[Option<Inner>]>,
        Outer::Buffer: std::convert::AsMut<[Option<Inner>]>,
        Inner::Buffer: std::convert::AsRef<[Option<CoolCell>]>,
        Inner::Buffer: std::convert::AsMut<[Option<CoolCell>]>,
    {
        use itertools::Itertools;
        self.into_inner()
            .into_iter()
            .map(|(_, v)| {
                v.into_iter()
                    .collect_tuple::<Inner>()
                    .expect("Can't destructure more than 4 DNAs")
            })
            .collect_tuple::<Outer>()
            .expect("Can't destructure more than 4 Agents")
    }
}

#[macro_export]
macro_rules! destructure_test_cell_vec {
    ($vec:expr) => {{
        use itertools::Itertools;
        let vec: Vec<$crate::test_utils::cool::CoolInstalledApps> = $vec;
        vec.into_iter()
            .map(|blob| blob.into_tuples())
            .collect_tuple()
            .expect("Can't destructure more than 4 Conductors")
    }};
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
            let shutdown = c.take_shutdown_handle().await.unwrap();
            c.shutdown().await;
            shutdown.await.unwrap();
        });
    }
}

// impl From<ConductorHandle> for CoolConductor {
//     fn from(h: ConductorHandle) -> Self {
//         CoolConductor(Arc::new(CoolConductorInner(h)))
//     }
// }
