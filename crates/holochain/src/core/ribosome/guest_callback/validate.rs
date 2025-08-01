use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_p2p::DynHolochainP2pDna;
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
    pub network: DynHolochainP2pDna,
    /// Whether this is an inline validation call.
    ///
    /// I.e. are we validating data that is being authored locally.
    pub is_inline: bool,
}

impl std::fmt::Debug for ValidateHostAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidateHostAccess").finish()
    }
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
    UnresolvedDependencies(UnresolvedDependencies),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for ValidateResult {
    /// This function is called after multiple app validation callbacks
    /// have been run by a Ribosome and it is necessary to return one
    /// decisive result to the host, even if that "decisive" result
    /// is the UnresolvedDependencies variant.
    /// It drops the irrelevant zome names and falls back to the conversion from
    /// a `Vec<ValidateCallbackResults>` -> ValidateResult
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
mod test {
    use super::ValidateInvocation;
    use super::ValidateResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidateHostAccessFixturator;
    use ::fixt::prelude::*;
    use holochain_types::prelude::*;
    use holochain_zome_types::op::Op;
    use rand::seq::SliceRandom;

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_valid = || ValidateResult::Valid;
        let result_ud =
            || ValidateResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(vec![]));
        let result_invalid = || ValidateResult::Invalid("".into());

        let cb_valid = || ValidateCallbackResult::Valid;
        let cb_ud = || {
            ValidateCallbackResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(vec![]))
        };
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
            let number_of_extras = rng.random_range(0..5);
            for _ in 0..number_of_extras {
                let maybe_extra = results.choose(&mut rng).cloned();
                if let Some(extra) = maybe_extra {
                    results.push(extra);
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
        let validate_invocation = ValidateInvocation::new(
            ZomesToInvoke::All,
            &Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::new_unchecked(
                    Action::CreateLink(fixt!(CreateLink)),
                    fixt!(Signature),
                ),
                cached_entry: None,
            }),
        )
        .unwrap();
        let zomes_to_invoke = validate_invocation.zomes_to_invoke.clone();
        assert_eq!(zomes_to_invoke, validate_invocation.zomes(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_fn_components() {
        let validate_invocation = ValidateInvocation::new(
            ZomesToInvoke::All,
            &Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::new_unchecked(
                    Action::CreateLink(fixt!(CreateLink)),
                    fixt!(Signature),
                ),
                cached_entry: None,
            }),
        )
        .unwrap();

        let mut expected = vec!["validate"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_invocation_host_input() {
        let op = Op::RegisterAgentActivity(RegisterAgentActivity {
            action: SignedActionHashed::new_unchecked(
                Action::CreateLink(fixt!(CreateLink)),
                fixt!(Signature),
            ),
            cached_entry: None,
        });
        let validate_invocation = ValidateInvocation::new(ZomesToInvoke::All, &op).unwrap();

        let host_input = validate_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(&op).unwrap(),);
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::ValidateResult;
    use crate::conductor::api::error::ConductorApiError;
    use crate::conductor::CellError;
    use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::RibosomeError;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::workflow::WorkflowError;
    use crate::fixt::Zomes;
    use crate::fixt::*;
    use crate::sweettest::{SweetConductor, SweetDnaFile};
    use ::fixt::prelude::*;
    use assert2::{assert, let_assert};
    use holochain_state::source_chain::SourceChainError;
    use holochain_types::inline_zome::InlineZomeSet;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::op::Op;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_unimplemented() {
        let validate_invocation = ValidateInvocation::new(
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::Foo).erase_type()),
            &Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::new_unchecked(
                    Action::CreateLink(fixt!(CreateLink)),
                    fixt!(Signature),
                ),
                cached_entry: None,
            }),
        )
        .unwrap();

        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .await
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_valid() {
        let validate_invocation = ValidateInvocation::new(
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::ValidateValid).erase_type()),
            &Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::new_unchecked(
                    Action::CreateLink(fixt!(CreateLink)),
                    fixt!(Signature),
                ),
                cached_entry: None,
            }),
        )
        .unwrap();

        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateValid]))
            .next()
            .unwrap();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .await
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_invalid_return() {
        let unit_entry_def = EntryDef::default_from_id("unit");
        let zome = InlineZomeSet::new_unique_single(
            "integrity",
            "coordinator",
            vec![unit_entry_def.clone()],
            0,
        )
        .function("integrity", "validate", |_api, _op: Op| Ok(42usize))
        .function("coordinator", "create", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        });

        let dnas = [SweetDnaFile::unique_from_inline_zomes(zome).await.0];
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", &dnas).await.unwrap();
        let conductor = Arc::new(conductor);
        let (cell,) = app.into_tuple();

        let err = conductor
            .call_fallible::<_, ()>(&cell.zome("coordinator"), "create", ())
            .await
            .unwrap_err();

        let_assert!(ConductorApiError::CellError(CellError::WorkflowError(workflow_err)) = err);

        let_assert!(
            WorkflowError::SourceChainError(SourceChainError::Other(other_err)) = *workflow_err
        );

        assert!(other_err
            .to_string()
            .contains("invalid value: integer `42`"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_invalid_params() {
        let validate_invocation = ValidateInvocation::new(
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::ValidateInvalidParams).erase_type()),
            &Op::RegisterAgentActivity(RegisterAgentActivity {
                action: SignedActionHashed::new_unchecked(
                    Action::CreateLink(fixt!(CreateLink)),
                    fixt!(Signature),
                ),
                cached_entry: None,
            }),
        )
        .unwrap();

        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalidParams]))
            .next()
            .unwrap();

        let err = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .await
            .unwrap_err();

        let_assert!(RibosomeError::CallbackInvalidParameters(err_msg) = err);
        assert!(err_msg == String::default());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_adding_entry_when_validate_implemented_invalid_params() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::ValidateInvalidParams).await;

        let err = conductor
            .call_fallible::<_, Record>(&alice, "create_entry_to_validate", ())
            .await
            .unwrap_err();

        let_assert!(ConductorApiError::CellError(CellError::WorkflowError(workflow_err)) = err);
        let_assert!(
            WorkflowError::SourceChainError(SourceChainError::Other(other_err)) = *workflow_err
        );
        // Can't downcast the `Box<dyn Error>` to a concrete type so just compare the error message.
        assert!(other_err.to_string() == "The callback has invalid parameters: ");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_multi() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();

        let agent = fixt!(AgentPubKey);
        let entry = Entry::Agent(agent.clone());
        let action = Create {
            author: agent.clone(),
            timestamp: Timestamp::now(),
            action_seq: 8,
            prev_action: fixt!(ActionHash),
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::with_data_sync(&entry),
            weight: EntryRateWeight::default(),
        };

        let op = Op::StoreRecord(StoreRecord {
            record: Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(action),
                    Signature(vec![7; SIGNATURE_BYTES].try_into().unwrap()),
                ),
                Some(entry),
            ),
        });

        let zomes_to_invoke =
            ZomesToInvoke::One(IntegrityZome::from(TestWasm::ValidateInvalid).erase_type());
        let validate_invocation = ValidateInvocation::new(zomes_to_invoke, &op).unwrap();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .await
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pass_validate_test() {
        holochain_trace::test_run();
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
