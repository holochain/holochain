use std::sync::Arc;

use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;

#[derive(Clone, Constructor, Debug)]
pub struct GenesisSelfCheckHostAccessV1;

#[derive(Clone)]
pub struct GenesisSelfCheckInvocationV1 {
    pub payload: Arc<GenesisSelfCheckDataV1>,
}

impl From<GenesisSelfCheckHostAccessV1> for HostContext {
    fn from(host_access: GenesisSelfCheckHostAccessV1) -> Self {
        Self::GenesisSelfCheckV1(host_access)
    }
}

impl From<&GenesisSelfCheckHostAccessV1> for HostFnAccess {
    fn from(_: &GenesisSelfCheckHostAccessV1) -> Self {
        let mut access = Self::none();
        access.keystore_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        access
    }
}

impl Invocation for GenesisSelfCheckInvocationV1 {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::AllIntegrity
    }
    fn fn_components(&self) -> FnComponents {
        // Backwards compatibility for callbacks implemented pre-versioning, as
        // well as support for explicit v1 extern.
        vec!["genesis_self_check".into(), "1".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.payload)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResultV1 {
    Valid,
    Invalid(String),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for GenesisSelfCheckResultV1 {
    fn from(a: Vec<(ZomeName, ValidateCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateCallbackResult>> for GenesisSelfCheckResultV1 {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCallbackResult::Invalid(i) => Self::Invalid(i),

                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,

                // this can't happen because self check has no DHT access.
                // don't want to panic so i guess it is invalid.
                ValidateCallbackResult::UnresolvedDependencies(_) => {
                    Self::Invalid(format!("{:?}", x))
                }
            }
        })
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub(crate) mod slow_tests {
    use std::sync::Arc;

    use super::GenesisSelfCheckInvocationV1;
    use crate::sweettest::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::{TestCoordinatorWasm, TestIntegrityWasm};

    pub(crate) fn invocation_fixture() -> GenesisSelfCheckInvocationV1 {
        GenesisSelfCheckInvocationV1 {
            payload: Arc::new(GenesisSelfCheckDataV1 {
                dna_info: fixt!(DnaInfoV1),
                membrane_proof: Some(Arc::new(().try_into().unwrap())),
                agent_key: fixt!(AgentPubKey),
            }),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_integrity_zome_can_run_self_check() {
        let mut conductor = SweetConductor::shared_rendezvous().await;
        let (dna, _, _) = SweetDnaFile::unique_from_zomes(
            vec![TestIntegrityWasm::IntegrityZome],
            Vec::<TestCoordinatorWasm>::new(),
            vec![TestIntegrityWasm::IntegrityZome],
        )
        .await;

        let app = conductor.setup_app("app", [&dna]).await.unwrap();
        let cells = app.into_cells();

        let _: EntryHashed = conductor
            .call(
                &cells[0].zome(TestIntegrityWasm::IntegrityZome),
                "call_must_get_entry",
                EntryHash::from(cells[0].cell_id().agent_pubkey().clone()),
            )
            .await;
    }
}
