use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holo_hash::AnyDhtHash;
use holochain_p2p::HolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_types::prelude::*;
use holochain_zome_types::op::Op;
use std::sync::Arc;

#[derive(Clone, Debug)]
/// An invocation of the validate callback function.
pub struct ValidateInvocation {
    /// The zomes this invocation will invoke.
    zomes_to_invoke: ZomesToInvoke,
    /// The serialized arguments to the callback function.
    data: Arc<ExternIO>,
}

impl ValidateInvocation {
    pub fn new(zomes_to_invoke: ZomesToInvoke, data: &Op) -> Result<Self, SerializedBytesError> {
        let data = Arc::new(ExternIO::encode(data)?);
        Ok(Self {
            zomes_to_invoke,
            data,
        })
    }
}

#[derive(Clone, Constructor)]
pub struct ValidateHostAccess {
    pub workspace: HostFnWorkspaceRead,
    pub network: HolochainP2pDna,
}

impl From<ValidateHostAccess> for HostContext {
    fn from(validate_host_access: ValidateHostAccess) -> Self {
        Self::Validate(validate_host_access)
    }
}

impl From<&ValidateHostAccess> for HostFnAccess {
    fn from(_: &ValidateHostAccess) -> Self {
        let mut access = Self::none();
        access.read_workspace_deterministic = Permission::Allow;
        access.keystore_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        access
    }
}

impl Invocation for ValidateInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        self.zomes_to_invoke.clone()
    }
    fn fn_components(&self) -> FnComponents {
        vec!["validate".to_string()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        // No option here but to clone the actual data as it's passed
        // into the host now anyway.
        Ok((*self.data).clone())
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for ValidateResult {
    /// This function is called after multiple app validation callbacks
    /// have been run by a Ribosome and it is necessary to return one
    /// decisive result to the host, even if that "decisive" result
    /// is the UnresolvedDependencies variant.
    /// It drops the irrelevant zome names and falls back to the conversion from
    /// a Vec<ValidateCallbackResults> -> ValidateResult
    fn from(a: Vec<(ZomeName, ValidateCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

/// if any ValidateCallbackResult is Invalid, then ValidateResult::Invalid
/// If none are Invalid and there is an UnresolvedDependencies, then ValidateResult::UnresolvedDependencies
/// If all ValidateCallbackResult are Valid, then ValidateResult::Valid
impl From<Vec<ValidateCallbackResult>> for ValidateResult {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Valid, |acc, x| match x {
                ValidateCallbackResult::Invalid(i) => Self::Invalid(i),
                ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud),
                },
                ValidateCallbackResult::Valid => acc,
            })
    }
}

#[cfg(test)]
impl<'a> arbitrary::Arbitrary<'a> for ValidateInvocation {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let zomes_to_invoke = ZomesToInvoke::arbitrary(u)?;
        let op = Op::arbitrary(u)?;
        Ok(Self::new(zomes_to_invoke, &op).unwrap())
    }
}

#[cfg(test)]
mod test {
    use super::ValidateInvocation;
    use super::ValidateResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidateHostAccessFixturator;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;
    use holochain_zome_types::op::Op;
    use rand::seq::SliceRandom;

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_valid = || ValidateResult::Valid;
        let result_ud = || ValidateResult::UnresolvedDependencies(vec![]);
        let result_invalid = || ValidateResult::Invalid("".into());

        let cb_valid = || ValidateCallbackResult::Valid;
        let cb_ud = || ValidateCallbackResult::UnresolvedDependencies(vec![]);
        let cb_invalid = || ValidateCallbackResult::Invalid("".into());

        for (mut results, expected) in vec![
            (vec![], result_valid()),
            (vec![cb_valid()], result_valid()),
            (vec![cb_invalid()], result_invalid()),
            (vec![cb_ud()], result_ud()),
            (vec![cb_invalid(), cb_valid()], result_invalid()),
            (vec![cb_invalid(), cb_ud()], result_invalid()),
            (vec![cb_valid(), cb_ud()], result_ud()),
            (vec![cb_valid(), cb_ud(), cb_invalid()], result_invalid()),
        ] {
            // order of the results should not change the final result
            results.shuffle(&mut rng);

            // number of times a callback result appears should not change the final result
            let number_of_extras = rng.gen_range(0..5);
            for _ in 0..number_of_extras {
                let maybe_extra = results.choose(&mut rng).cloned();
                match maybe_extra {
                    Some(extra) => results.push(extra),
                    _ => {}
                };
            }

            assert_eq!(expected, results.into(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_allow_side_effects() {
        let validate_host_access = ValidateHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let mut access = HostFnAccess::none();
        access.read_workspace_deterministic = Permission::Allow;
        access.keystore_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        assert_eq!(HostFnAccess::from(&validate_host_access), access);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_zomes() {
        let mut u = Unstructured::new(&NOISE);
        let validate_invocation = ValidateInvocation::arbitrary(&mut u).unwrap();
        let zomes_to_invoke = validate_invocation.zomes_to_invoke.clone();
        assert_eq!(zomes_to_invoke, validate_invocation.zomes(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_fn_components() {
        let mut u = Unstructured::new(&NOISE);
        let validate_invocation = ValidateInvocation::arbitrary(&mut u).unwrap();

        let mut expected = vec!["validate"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_host_input() {
        let mut u = Unstructured::new(&NOISE);
        let op = Op::arbitrary(&mut u).unwrap();
        let validate_invocation =
            ValidateInvocation::new(ZomesToInvoke::arbitrary(&mut u).unwrap(), &op).unwrap();

        let host_input = validate_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(&op).unwrap(),);
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::ValidateResult;
    use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::op::Op;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_unimplemented() {
        let mut u = Unstructured::new(&NOISE);
        let mut validate_invocation = ValidateInvocation::arbitrary(&mut u).unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        validate_invocation.zomes_to_invoke =
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::Foo).erase_type());

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_valid() {
        let mut u = Unstructured::new(&NOISE);
        let mut validate_invocation = ValidateInvocation::arbitrary(&mut u).unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateValid]))
            .next()
            .unwrap();
        validate_invocation.zomes_to_invoke =
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::ValidateValid).erase_type());

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_multi() {
        let mut u = Unstructured::new(&NOISE);

        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();

        let agent = AgentPubKey::arbitrary(&mut u).unwrap();
        let entry = Entry::Agent(agent);
        let mut action = Create::arbitrary(&mut u).unwrap();
        action.entry_type = EntryType::AgentPubKey;
        action.entry_hash = EntryHash::with_data_sync(&entry);

        let op = Op::StoreRecord(StoreRecord {
            record: Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action.into()),
                    Signature::arbitrary(&mut u).unwrap(),
                ),
                Some(entry),
            ),
        });

        let zomes_to_invoke =
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::ValidateInvalid).erase_type());
        let validate_invocation = ValidateInvocation::new(zomes_to_invoke, &op).unwrap();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pass_validate_test() {
        observability::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::Validate).await;

        let output: ActionHash = conductor.call(&alice, "always_validates", ()).await;
        let _output_record: Record = conductor
            .call(&alice, "must_get_valid_record", output)
            .await;

        let invalid_output: Result<ActionHash, _> =
            conductor.call_fallible(&alice, "never_validates", ()).await;
        assert!(invalid_output.is_err());
    }
}
