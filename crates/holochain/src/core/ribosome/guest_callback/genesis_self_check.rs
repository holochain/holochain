pub mod v1;
pub mod v2;

use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckResultV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckResultV2;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::GenesisSelfCheckDataV1;
use holochain_zome_types::GenesisSelfCheckDataV2;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GenesisSelfCheckResult {
    Valid,
    Invalid(String),
}

impl From<GenesisSelfCheckResultV1> for GenesisSelfCheckResult {
    fn from(result_v1: GenesisSelfCheckResultV1) -> Self {
        match result_v1 {
            GenesisSelfCheckResultV1::Valid => Self::Valid,
            GenesisSelfCheckResultV1::Invalid(s) => Self::Invalid(s),
        }
    }
}

impl From<GenesisSelfCheckResultV2> for GenesisSelfCheckResult {
    fn from(result_v2: GenesisSelfCheckResultV2) -> Self {
        match result_v2 {
            GenesisSelfCheckResultV2::Valid => Self::Valid,
            GenesisSelfCheckResultV2::Invalid(s) => Self::Invalid(s),
        }
    }
}

#[derive(Clone, Constructor, Debug)]
pub struct GenesisSelfCheckHostAccess {
    pub host_access_1: GenesisSelfCheckHostAccessV1,
    pub host_access_2: GenesisSelfCheckHostAccessV2,
}

impl From<GenesisSelfCheckHostAccess>
    for (GenesisSelfCheckHostAccessV1, GenesisSelfCheckHostAccessV2)
{
    fn from(invocation: GenesisSelfCheckHostAccess) -> Self {
        (invocation.host_access_1, invocation.host_access_2)
    }
}

#[derive(Clone)]
pub struct GenesisSelfCheckInvocation {
    pub data_1: Arc<GenesisSelfCheckDataV1>,
    pub data_2: Arc<GenesisSelfCheckDataV2>,
}

impl From<GenesisSelfCheckInvocation>
    for (GenesisSelfCheckInvocationV1, GenesisSelfCheckInvocationV2)
{
    fn from(invocation: GenesisSelfCheckInvocation) -> Self {
        (
            GenesisSelfCheckInvocationV1 {
                payload: invocation.data_1,
            },
            GenesisSelfCheckInvocationV2 {
                payload: invocation.data_2,
            },
        )
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::GenesisSelfCheckInvocation;
    use crate::core::ribosome::GenesisSelfCheckHostAccessV1;
    use crate::core::ribosome::GenesisSelfCheckHostAccessV2;
    use holochain_wasm_test_utils::TestWasm;
    use crate::{
        core::ribosome::{
            guest_callback::genesis_self_check::{
                GenesisSelfCheckHostAccess, GenesisSelfCheckResult,
            },
            RibosomeT,
        },
    };
    use crate::fixt::curve::Zomes;
    use crate::fixt::RealRibosomeFixturator;
    use super::v1;
    use super::v2;
    use std::sync::Arc;

    fn invocation_fixture() -> GenesisSelfCheckInvocation {
        GenesisSelfCheckInvocation {
            data_1: Arc::new(v1::slow_tests::invocation_fixture()),
            data_2: Arc::new(v2::slow_tests::invocation_fixture()),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_genesis_self_check_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(GenesisSelfCheckHostAccess {
                host_access_1: GenesisSelfCheckHostAccessV1,
                host_access_2: GenesisSelfCheckHostAccessV2,
            }, invocation)
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
            .run_genesis_self_check(GenesisSelfCheckHostAccess {
                host_access_1: GenesisSelfCheckHostAccessV1,
                host_access_2: GenesisSelfCheckHostAccessV2,
            }, invocation)
            .unwrap();
        assert_eq!(
            result,
            GenesisSelfCheckResult::Invalid("esoteric edge case".into()),
        );
    }
}