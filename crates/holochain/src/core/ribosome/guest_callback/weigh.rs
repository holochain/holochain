use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::rate_limit::WeighCallbackResult;

#[derive(Clone, Debug)]
/// An invocation of the weigh callback function.
pub struct WeighInvocation {
    /// The zome this invocation will invoke.
    zome: IntegrityZome,
    /// The thing to be weighed.
    input: WeighInput,
}

impl WeighInvocation {
    pub fn new(zome: IntegrityZome, input: WeighInput) -> Self {
        Self { zome, input }
    }
}

#[derive(Clone, Constructor)]
pub struct WeighHostAccess {}

impl From<WeighHostAccess> for HostContext {
    fn from(access: WeighHostAccess) -> Self {
        Self::Weigh(access)
    }
}

impl From<&WeighHostAccess> for HostFnAccess {
    fn from(_: &WeighHostAccess) -> Self {
        Self::none()
    }
}

impl Invocation for WeighInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::OneIntegrity(self.zome.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["weigh".to_string()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.input)
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

#[derive(Clone, PartialEq, Debug, derive_more::Deref, Default)]
pub struct WeighResult(WeighCallbackResult);

impl From<WeighResult> for RateWeight {
    fn from(r: WeighResult) -> Self {
        r.0.into()
    }
}

impl From<Vec<(ZomeName, WeighCallbackResult)>> for WeighResult {
    fn from(mut results: Vec<(ZomeName, WeighCallbackResult)>) -> Self {
        assert!(
            results.len() < 2,
            "There will never be more than one weigh() callback invoked at a time."
        );
        // If the `weigh()` callback could not be found, then use default weight
        // (zero weight, bucket 255)
        Self(results.pop().map(|(_, w)| w).unwrap_or_default())
    }
}

#[cfg(test)]
impl<'a> arbitrary::Arbitrary<'a> for WeighInvocation {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let zome = IntegrityZome::arbitrary(u)?;
        let input = WeighInput::arbitrary(u)?;
        Ok(Self::new(zome, input))
    }
}

#[cfg(test)]
mod test {
    use super::WeighInvocation;
    use crate::core::ribosome::Invocation;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn weigh_invocation_fn_components() {
        let mut u = Unstructured::new(&NOISE);
        let weigh_invocation = WeighInvocation::arbitrary(&mut u).unwrap();

        let mut expected = vec!["weigh"];
        for fn_component in weigh_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn weigh_invocation_host_input() {
        let mut u = Unstructured::new(&NOISE);
        let input = WeighInput::arbitrary(&mut u).unwrap();
        let weigh_invocation =
            WeighInvocation::new(IntegrityZome::arbitrary(&mut u).unwrap(), input.clone());

        let host_input = weigh_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(&input).unwrap(),);
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::WeighResult;
    use crate::core::ribosome::guest_callback::weigh::WeighInvocation;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::rate_limit::WeighCallbackResult;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_weigh_unimplemented() {
        let mut u = Unstructured::new(&NOISE);
        let mut weigh_invocation = WeighInvocation::arbitrary(&mut u).unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        weigh_invocation.zome = TestWasm::Foo.into();

        let result = ribosome.run_weigh(weigh_invocation).unwrap();
        assert_eq!(result, WeighResult::default(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_weigh_implemented_valid() {
        let mut u = Unstructured::new(&NOISE);
        let weigh_invocation = WeighInvocation {
            zome: TestWasm::Weigh.into(),
            input: WeighInput::Create(
                Create::arbitrary(&mut u).unwrap(),
                Entry::arbitrary(&mut u).unwrap(),
            ),
        };
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Weigh]))
            .next()
            .unwrap();

        let result = ribosome.run_weigh(weigh_invocation).unwrap();
        assert_eq!(
            result,
            WeighResult(WeighCallbackResult {
                bucket_id: 3,
                units: 2
            }),
        );
    }
}
