//! A wrapper around ConductorHandle with more convenient methods for testing
// TODO [ B-03669 ] move to own crate

use super::test_cell::TestCell;
use crate::conductor::api::ZomeCall;
use crate::conductor::handle::ConductorHandle;
use crate::core::ribosome::ZomeCallInvocation;
use hdk::prelude::*;
use holochain_types::prelude::*;
use unwrap_to::unwrap_to;

/// A wrapper around ConductorHandle with more convenient methods for testing.
#[derive(Clone, shrinkwraprs::Shrinkwrap, derive_more::From)]
pub struct TestConductorHandle(pub(crate) ConductorHandle);

impl TestConductorHandle {
    /// Opinionated app setup. Creates one app per agent, using the given DnaFiles.
    ///
    /// All InstalledAppIds and AppRoleIds are auto-generated. In tests driven directly
    /// by Rust, you typically won't care what these values are set to, but in case you
    /// do, they are set as so:
    /// - InstalledAppId: {app_id_prefix}-{agent_pub_key}
    /// - AppRoleId: {dna_hash}
    ///
    /// Returns the list of generated InstalledAppIds, in the same order as Agents passed in.
    pub async fn setup_app_for_agents_with_no_membrane_proof(
        &self,
        app_id_prefix: &str,
        agents: &[AgentPubKey],
        dna_files: &[DnaFile],
    ) -> SetupOutput {
        for dna_file in dna_files {
            self.0
                .register_dna(dna_file.clone())
                .await
                .expect("Could not install DNA")
        }

        let mut info = Vec::new();

        for agent in agents {
            let installed_app_id = format!("{}{}", app_id_prefix, agent);
            let cell_ids: Vec<TestCell> = dna_files
                .iter()
                .map(|f| CellId::new(f.dna_hash().clone(), agent.clone()))
                .map(|cell_id| TestCell {
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
            self.0
                .clone()
                .install_app(installed_app_id.clone(), cells)
                .await
                .expect("Could not install app");
            info.push((installed_app_id, cell_ids));
        }

        for (installed_app_id, _) in info.iter() {
            self.0
                .activate_app(installed_app_id.clone())
                .await
                .expect("Could not activate app");
        }

        self.0
            .clone()
            .setup_cells()
            .await
            .expect("Could not setup cells");

        info
    }
}

/// Return type of opinionated setup function
pub type SetupOutput = Vec<(InstalledAppId, Vec<TestCell>)>;

/// Helper to destructure the nested app setup return value as nested tuples.
/// Each level of nesting can contain 1-4 items, i.e. up to 4 agents with 4 DNAs each.
/// Beyond 4, and this will PANIC! (But it's just for tests so it's fine.)
#[macro_export]
macro_rules! destructure_test_cells {
    ($blob:expr) => {{
        use $crate::test_utils::itertools::Itertools;
        let blob: $crate::test_utils::test_conductor::SetupOutput = $blob;
        blob.into_iter()
            .map(|(_, v)| {
                v.into_iter()
                    .collect_tuple()
                    .expect("Wrong number of DNAs in destructuring pattern, or too many (must be 4 or less)")
            })
            .collect_tuple()
            .expect("Wrong number of Agents in destructuring pattern, or too many (must be 4 or less)")
    }};
}
#[macro_export]
macro_rules! destructure_test_cell_vec {
    ($vec:expr) => {{
        use $crate::test_utils::itertools::Itertools;
        let vec: Vec<$crate::test_utils::test_conductor::SetupOutput> = $vec;
        vec.into_iter()
            .map(|blob| destructure_test_cells!(blob))
            .collect_tuple()
            .expect("Wrong number of Conductors in destructuring pattern, or too many (must be 4 or less)")
    }};
}

impl TestConductorHandle {
    /// Call a zome function with automatic de/serialization of input and output
    /// and unwrapping of nested errors.
    pub async fn call_zome_ok<'a, I, O, F, E>(&'a self, invocation: TestZomeCall<'a, I, F, E>) -> O
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
    pub async fn call_zome_ok_flat<I, O, Z, F>(
        &self,
        cell_id: &CellId,
        zome_name: Z,
        fn_name: F,
        cap_secret: Option<CapSecret>,
        provenance: Option<AgentPubKey>,
        payload: I,
    ) -> O
    where
        ZomeName: From<Z>,
        FunctionName: From<F>,
        I: Serialize,
        O: DeserializeOwned + std::fmt::Debug,
    {
        let payload = ExternIO::encode(payload).expect("Couldn't serialize payload");
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        let call = ZomeCall {
            cell_id: cell_id.clone(),
            zome_name: zome_name.into(),
            fn_name: fn_name.into(),
            cap_secret,
            provenance,
            payload,
        };
        let response = self.0.call_zome(call).await.unwrap().unwrap();
        unwrap_to!(response => ZomeCallResponse::Ok)
            .decode()
            .expect("Couldn't deserialize zome call output")
    }
}

/// A top-level call into a zome function,
/// i.e. coming from outside the Cell from an external Interface
#[derive(Clone, Debug)]
pub struct TestZomeCall<'a, P, F, E>
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
    pub cap_secret: Option<CapSecret>,
    /// The name of the Zome function to call
    pub fn_name: F,
    /// The data to be serialized and passed as an argument to the Zome call
    pub payload: P,
    /// If None, the AgentPubKey from the CellId is used (a common case)
    pub provenance: Option<AgentPubKey>,
}

impl<'a, P, F> From<TestZomeCall<'a, P, F>> for ZomeCallInvocation
where
    P: Serialize,
    FunctionName: From<F>,
{
    fn from(tzci: TestZomeCall<'a, P, F>) -> Self {
        let TestZomeCall {
            cell_id,
            zome,
            fn_name,
            cap_secret,
            provenance,
            payload,
        } = tzci;
        let payload = ExternIO::encode(payload).expect("Couldn't serialize payload");
        let provenance = provenance.unwrap_or_else(|| cell_id.agent_pubkey().clone());
        ZomeCallInvocation {
            cell_id: cell_id.clone(),
            zome: zome.clone(),
            fn_name: fn_name.into(),
            cap_secret,
            provenance,
            payload,
        }
    }
}

impl<'a, P, F, E> From<TestZomeCall<'a, P, F, E>> for ZomeCall
where
    SerializedBytes: TryFrom<P, Error = E>,
    E: std::fmt::Debug,
    FunctionName: From<F>,
{
    fn from(tzci: TestZomeCall<'a, P, F, E>) -> Self {
        ZomeCallInvocation::from(tzci).into()
    }
}
