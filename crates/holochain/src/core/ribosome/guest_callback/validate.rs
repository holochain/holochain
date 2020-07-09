use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::EntryFixturator;
use crate::fixt::ZomeNameFixturator;
use fixt::prelude::*;
use holo_hash::EntryContentHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::sync::Arc;
use holochain_types::dna::zome::HostFnAccess;

#[derive(Clone)]
pub struct ValidateInvocation {
    pub zome_name: ZomeName,
    // Arc here as entry may be very large
    // don't want to clone the Entry just to validate it
    // we can SerializedBytes off an Entry reference
    // lifetimes on invocations are a pain
    pub entry: Arc<Entry>,
}

impl ValidateInvocation {
    pub fn new(zome_name: ZomeName, entry: Entry) -> Self {
        Self {
            zome_name,
            entry: Arc::new(entry),
        }
    }
}

fixturator!(
    ValidateInvocation;
    constructor fn new(ZomeName, Entry);
);

impl Invocation for ValidateInvocation {
    fn allowed_access(&self) -> HostFnAccess {
        HostFnAccess::none()
    }
    fn zomes(&self) -> ZomesToInvoke {
        // entries are specific to zomes so only validate in the zome the entry is defined in
        // note that here it is possible there is a zome/entry mismatch
        // we rely on the invocation to be built correctly
        ZomesToInvoke::One(self.zome_name.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "validate".into(),
            match *self.entry {
                Entry::Agent(_) => "agent",
                Entry::App(_) => "entry",
                Entry::CapClaim(_) => "cap_claim",
                Entry::CapGrant(_) => "cap_grant",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&*self.entry).try_into()?))
    }
}

impl TryFrom<ValidateInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(validate_invocation: ValidateInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&*validate_invocation.entry).try_into()?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryContentHash>),
}

impl From<Vec<ValidateCallbackResult>> for ValidateResult {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCallbackResult::Invalid(i) => Self::Invalid(i),
                // return unresolved dependencies if it's otherwise valid
                ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud.into_iter().map(|h| h.into()).collect()),
                },
                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,
            }
        })
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod test {

    use super::ValidateInvocationFixturator;
    use super::ValidateResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallCapGrantFixturator;
    use fixt::prelude::*;
    use holo_hash::AgentPubKeyFixturator;
    use holo_hash_core::HoloHashCoreHash;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::fixt::CapClaimFixturator;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::validate::ValidateCallbackResult;
    use holochain_zome_types::CommitEntryOutput;
    use holochain_zome_types::HostInput;
    use rand::seq::SliceRandom;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn validate_callback_result_fold() {
        let mut rng = thread_rng();

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
            let number_of_extras = rng.gen_range(0, 5);
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

    #[tokio::test(threaded_scheduler)]
    async fn validate_invocation_allow_side_effects() {
        let validate_invocation = ValidateInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert!(!validate_invocation.allow_side_effects());
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_invocation_zomes() {
        let validate_invocation = ValidateInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let zome_name = validate_invocation.zome_name.clone();
        assert_eq!(ZomesToInvoke::One(zome_name), validate_invocation.zomes(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_invocation_fn_components() {
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let agent_entry = Entry::Agent(
            AgentPubKeyFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );
        validate_invocation.entry = Arc::new(agent_entry);
        let mut expected = vec!["validate", "validate_agent"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }

        let agent_entry = Entry::App(
            SerializedBytesFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );
        validate_invocation.entry = Arc::new(agent_entry);
        let mut expected = vec!["validate", "validate_entry"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }

        let agent_entry = Entry::CapClaim(
            CapClaimFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );
        validate_invocation.entry = Arc::new(agent_entry);
        let mut expected = vec!["validate", "validate_cap_claim"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }

        let agent_entry = Entry::CapGrant(
            ZomeCallCapGrantFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );
        validate_invocation.entry = Arc::new(agent_entry);
        let mut expected = vec!["validate", "validate_cap_grant"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_invocation_host_input() {
        let validate_invocation = ValidateInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = validate_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(SerializedBytes::try_from(&*validate_invocation.entry).unwrap()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validate_unimplemented() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_validate(workspace, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validate_implemented_valid() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateValid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateValid.into();

        let result = ribosome
            .run_validate(workspace, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validate_implemented_invalid() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateInvalid.into();

        let result = ribosome
            .run_validate(workspace, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()),);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validate_implemented_multi() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        let entry = Entry::Agent(
            AgentPubKeyFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );

        validate_invocation.zome_name = TestWasm::ValidateInvalid.into();
        validate_invocation.entry = Arc::new(entry);

        let result = ribosome
            .run_validate(workspace, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()));
    }

    #[tokio::test(threaded_scheduler)]
    async fn pass_validate_test() {
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let output: CommitEntryOutput =
            crate::call_test_ribosome!(raw_workspace, TestWasm::Validate, "always_validates", ());

        assert_eq!(
            vec![
                65, 163, 251, 163, 192, 168, 221, 213, 231, 24, 5, 83, 106, 135, 117, 197, 241, 60,
                21, 12, 68, 95, 184, 246, 149, 236, 172, 56, 91, 253, 174, 12, 149, 48, 124, 63
            ]
            .as_slice(),
            output.into_inner().get_raw(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn fail_validate_test() {
        // test workspace boilerplate
        let env = holochain_state::test_utils::test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
        let mut workspace = <crate::core::workflow::call_zome_workflow::InvokeZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let (_g, raw_workspace) = crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

        let output: CommitEntryOutput =
            crate::call_test_ribosome!(raw_workspace, TestWasm::Validate, "never_validates", ());

        assert_eq!(
            vec![
                76, 230, 153, 63, 221, 14, 217, 80, 6, 139, 12, 225, 82, 74, 160, 244, 168, 172,
                79, 168, 122, 95, 86, 33, 1, 98, 133, 173, 215, 49, 252, 75, 200, 146, 2, 126
            ]
            .as_slice(),
            output.into_inner().get_raw(),
        );
    }
}
