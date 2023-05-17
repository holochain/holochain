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
pub struct GenesisSelfCheckHostAccessV2;

#[derive(Clone)]
pub struct GenesisSelfCheckInvocationV2 {
    pub payload: Arc<GenesisSelfCheckDataV2>
}

impl From<GenesisSelfCheckHostAccessV2> for HostContext {
    fn from(host_access: GenesisSelfCheckHostAccessV2) -> Self {
        Self::GenesisSelfCheckV2(host_access)
    }
}

impl Invocation for GenesisSelfCheckInvocationV2 {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::AllIntegrity
    }
    fn fn_components(&self) -> FnComponents {
        vec!["genesis_self_check_2".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.payload)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResultV2 {
    Valid,
    Invalid(String),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for GenesisSelfCheckResultV2 {
    fn from(a: Vec<(ZomeName, ValidateCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateCallbackResult>> for GenesisSelfCheckResultV2 {
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
mod slow_tests {
    use std::sync::Arc;

    use super::GenesisSelfCheckInvocation;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use crate::{
        core::ribosome::{
            guest_callback::genesis_self_check::{
                GenesisSelfCheckHostAccess, GenesisSelfCheckResult,
            },
            RibosomeT,
        },
        sweettest::*,
    };
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::{TestCoordinatorWasm, TestIntegrityWasm, TestWasm};

    fn invocation_fixture() -> GenesisSelfCheckInvocation {
        GenesisSelfCheckInvocation {
            payload: Arc::new(GenesisSelfCheckData {
                dna_info: fixt!(DnaInfo),
                membrane_proof: Some(Arc::new(().try_into().unwrap())),
                agent_key: fixt!(AgentPubKey),
            }),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(GenesisSelfCheckHostAccess, invocation)
            .unwrap();
        assert_eq!(result, GenesisSelfCheckResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_valid() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::GenesisSelfCheckValid]))
            .next()
            .unwrap();
        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(GenesisSelfCheckHostAccess, invocation)
            .unwrap();
        assert_eq!(result, GenesisSelfCheckResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_invalid() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::GenesisSelfCheckInvalid]))
            .next()
            .unwrap();

        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(GenesisSelfCheckHostAccess, invocation)
            .unwrap();
        assert_eq!(
            result,
            GenesisSelfCheckResult::Invalid("esoteric edge case".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_integrity_zome_can_run_self_check() {
        let mut conductor = SweetConductor::from_standard_config().await;
        let (dna, _, _) = SweetDnaFile::unique_from_zomes(
            vec![TestIntegrityWasm::IntegrityZome],
            Vec::<TestCoordinatorWasm>::new(),
            vec![TestIntegrityWasm::IntegrityZome],
        )
        .await;

        let app = conductor.setup_app("app", &[dna]).await.unwrap();
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
