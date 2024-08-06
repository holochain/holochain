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
    pub invocation_1: GenesisSelfCheckInvocationV1,
    pub invocation_2: GenesisSelfCheckInvocationV2,
}

impl From<GenesisSelfCheckInvocation>
    for (GenesisSelfCheckInvocationV1, GenesisSelfCheckInvocationV2)
{
    fn from(invocation: GenesisSelfCheckInvocation) -> Self {
        (invocation.invocation_1, invocation.invocation_2)
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::v1;
    use super::v2;
    use super::GenesisSelfCheckInvocation;
    use crate::core::ribosome::GenesisSelfCheckHostAccessV1;
    use crate::core::ribosome::GenesisSelfCheckHostAccessV2;
    use crate::core::ribosome::{
        guest_callback::genesis_self_check::{GenesisSelfCheckHostAccess, GenesisSelfCheckResult},
        RibosomeT,
    };
    use crate::fixt::curve::Zomes;
    use crate::fixt::RealRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;

    fn invocation_fixture() -> GenesisSelfCheckInvocation {
        GenesisSelfCheckInvocation {
            invocation_1: v1::slow_tests::invocation_fixture(),
            invocation_2: v2::slow_tests::invocation_fixture(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_genesis_self_check_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(
                GenesisSelfCheckHostAccess {
                    host_access_1: GenesisSelfCheckHostAccessV1,
                    host_access_2: GenesisSelfCheckHostAccessV2,
                },
                invocation,
            )
            .await
            .unwrap();
        assert_eq!(result, GenesisSelfCheckResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_genesis_self_check_implemented_invalid() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::GenesisSelfCheckInvalid]))
            .next()
            .unwrap();

        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(
                GenesisSelfCheckHostAccess {
                    host_access_1: GenesisSelfCheckHostAccessV1,
                    host_access_2: GenesisSelfCheckHostAccessV2,
                },
                invocation,
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            GenesisSelfCheckResult::Invalid("esoteric edge case".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_genesis_self_check_implemented_valid() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::GenesisSelfCheckValid]))
            .next()
            .unwrap();

        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(
                GenesisSelfCheckHostAccess {
                    host_access_1: GenesisSelfCheckHostAccessV1,
                    host_access_2: GenesisSelfCheckHostAccessV2,
                },
                invocation,
            )
            .await
            .unwrap();
        assert_eq!(result, GenesisSelfCheckResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_genesis_self_check_implemented_valid_legacy() {
        let ribosome =
            RealRibosomeFixturator::new(Zomes(vec![TestWasm::GenesisSelfCheckValidLegacy]))
                .next()
                .unwrap();

        let invocation = invocation_fixture();

        let result = ribosome
            .run_genesis_self_check(
                GenesisSelfCheckHostAccess {
                    host_access_1: GenesisSelfCheckHostAccessV1,
                    host_access_2: GenesisSelfCheckHostAccessV2,
                },
                invocation,
            )
            .await
            .unwrap();
        assert_eq!(result, GenesisSelfCheckResult::Valid,);
    }
}
