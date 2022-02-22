use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;

#[derive(Clone)]
pub struct GenesisSelfCheckInvocation {
    pub payload: GenesisSelfCheckData,
}

#[derive(Clone, Constructor)]
pub struct GenesisSelfCheckHostAccess;

impl From<GenesisSelfCheckHostAccess> for HostContext {
    fn from(host_access: GenesisSelfCheckHostAccess) -> Self {
        Self::GenesisSelfCheck(host_access)
    }
}

impl From<&GenesisSelfCheckHostAccess> for HostFnAccess {
    fn from(_: &GenesisSelfCheckHostAccess) -> Self {
        let mut access = Self::none();
        access.keystore_deterministic = Permission::Allow;
        access
    }
}

impl Invocation for GenesisSelfCheckInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
    }
    fn fn_components(&self) -> FnComponents {
        vec!["genesis_self_check".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.payload)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

impl From<GenesisSelfCheckInvocation> for GenesisSelfCheckData {
    fn from(i: GenesisSelfCheckInvocation) -> Self {
        i.payload
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResult {
    Valid,
    Invalid(String),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for GenesisSelfCheckResult {
    fn from(a: Vec<(ZomeName, ValidateCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateCallbackResult>> for GenesisSelfCheckResult {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCallbackResult::Invalid(i) => GenesisSelfCheckResult::Invalid(i),

                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,

                // this can't happen because self check has no DHT access.
                // don't want to panic so i guess it is invalid.
                ValidateCallbackResult::UnresolvedDependencies(_) => {
                    GenesisSelfCheckResult::Invalid(format!("{:?}", x))
                }
            }
        })
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::GenesisSelfCheckInvocation;
    use crate::core::ribosome::{
        guest_callback::genesis_self_check::{GenesisSelfCheckHostAccess, GenesisSelfCheckResult},
        RibosomeT,
    };
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    fn invocation_fixture() -> GenesisSelfCheckInvocation {
        GenesisSelfCheckInvocation {
            payload: GenesisSelfCheckData {
                dna_def: fixt!(DnaDef),
                membrane_proof: Some(().try_into().unwrap()),
                agent_key: fixt!(AgentPubKey),
            },
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
}
