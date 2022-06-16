use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::rate_limit::RateLimitsCallbackResult;

#[derive(Clone, Debug)]
/// An invocation of the rate_limits callback function.
pub struct RateLimitsInvocation {
    /// The zome this invocation will invoke.
    zome: IntegrityZome,
}

impl RateLimitsInvocation {
    pub fn new(zome: IntegrityZome) -> Self {
        Self { zome }
    }
}

#[derive(Clone, Constructor)]
pub struct RateLimitsHostAccess {}

impl From<RateLimitsHostAccess> for HostContext {
    fn from(access: RateLimitsHostAccess) -> Self {
        Self::RateLimits(access)
    }
}

impl From<&RateLimitsHostAccess> for HostFnAccess {
    fn from(_: &RateLimitsHostAccess) -> Self {
        Self::none()
    }
}

impl Invocation for RateLimitsInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::OneIntegrity(self.zome.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["rate_limits".to_string()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(())
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

#[derive(Clone, PartialEq, Debug, derive_more::Deref, Default)]
pub struct RateLimitsResult(RateLimitsCallbackResult);

impl From<Vec<(ZomeName, RateLimitsCallbackResult)>> for RateLimitsResult {
    fn from(mut results: Vec<(ZomeName, RateLimitsCallbackResult)>) -> Self {
        assert!(
            results.len() < 2,
            "There will never be more than one rate_limits() callback invoked at a time."
        );
        // If the `rate_limits()` callback could not be found, then use default rate_limits
        // (zero rate_limits, bucket 255)
        Self(results.pop().map(|(_, w)| w).unwrap_or_default())
    }
}

#[cfg(test)]
impl<'a> arbitrary::Arbitrary<'a> for RateLimitsInvocation {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let zome = IntegrityZome::arbitrary(u)?;
        Ok(Self::new(zome))
    }
}

#[cfg(test)]
mod test {
    use super::RateLimitsInvocation;
    use crate::core::ribosome::Invocation;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn rate_limits_invocation_fn_components() {
        let mut u = Unstructured::new(&NOISE);
        let rate_limits_invocation = RateLimitsInvocation::arbitrary(&mut u).unwrap();

        let mut expected = vec!["rate_limits"];
        for fn_component in rate_limits_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rate_limits_invocation_host_input() {
        let mut u = Unstructured::new(&NOISE);
        let rate_limits_invocation =
            RateLimitsInvocation::new(IntegrityZome::arbitrary(&mut u).unwrap());

        let host_input = rate_limits_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(&()).unwrap());
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::RateLimitsResult;
    use crate::core::ribosome::guest_callback::rate_limits::RateLimitsInvocation;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::rate_limit::RateLimitsCallbackResult;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limits_unimplemented() {
        let mut u = Unstructured::new(&NOISE);
        let mut rate_limits_invocation = RateLimitsInvocation::arbitrary(&mut u).unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        rate_limits_invocation.zome = TestWasm::Foo.into();

        let result = ribosome.run_rate_limits(rate_limits_invocation).unwrap();
        assert_eq!(result, RateLimitsResult::default(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limits_implemented_valid() {
        let rate_limits_invocation = RateLimitsInvocation {
            zome: TestWasm::RateLimits.into(),
        };
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::RateLimits]))
            .next()
            .unwrap();

        let result = ribosome.run_rate_limits(rate_limits_invocation).unwrap();
        assert_eq!(
            result,
            RateLimitsResult(RateLimitsCallbackResult::new(vec![RateLimit {
                capacity: 1000,
                drain_amount: 100,
                drain_interval_ms: 10,
            }])),
        );
    }
}
