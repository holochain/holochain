use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::workflow::CallZomeWorkspaceLock;
use derive_more::Constructor;
use holo_hash::AnyDhtHash;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct GenesisSelfCheckInvocation {
    pub payload: GenesisSelfCheckData,
}

#[derive(Clone, Constructor)]
pub struct GenesisSelfCheckHostAccess;

impl From<GenesisSelfCheckHostAccess> for HostAccess {
    fn from(host_access: GenesisSelfCheckHostAccess) -> Self {
        Self::GenesisSelfCheck(host_access)
    }
}

impl From<&GenesisSelfCheckHostAccess> for HostFnAccess {
    fn from(_: &GenesisSelfCheckHostAccess) -> Self {
        let access = Self::none();
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

impl CallbackResult for GenesisSelfCheckResult {
    fn is_definitive(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }
}

impl From<ExternIO> for GenesisSelfCheckResult {
    fn from(guest_output: ExternIO) -> Self {
        match guest_output.decode() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}

impl From<Vec<(ZomeName, GenesisSelfCheckResult)>> for GenesisSelfCheckResult {
    fn from(a: Vec<(ZomeName, GenesisSelfCheckResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<GenesisSelfCheckResult>> for GenesisSelfCheckResult {
    fn from(callback_results: Vec<GenesisSelfCheckResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                GenesisSelfCheckResult::Invalid(i) => Self::Invalid(i),

                // valid x allows validation to continue
                GenesisSelfCheckResult::Valid => acc,
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
