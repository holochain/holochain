use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::{ribosome::FnComponents, workflow::CallZomeWorkspaceLock};
use derive_more::Constructor;
use holo_hash::EntryHash;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::zome::{HostFnAccess, Permission};
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use holochain_zome_types::{element::Element, validate::ValidationPackage, Header};
use holochain_zome_types::{entry::Entry, validate::ValidateData};
use std::sync::Arc;

#[derive(Clone)]
pub struct ValidateInvocation {
    pub zome_name: ZomeName,
    // Arc here as entry may be very large
    // don't want to clone the Element just to validate it
    // we can SerializedBytes off an Element reference
    // lifetimes on invocations are a pain
    pub element: Arc<Element>,
    /// Only elements with an app entry
    /// will have a validation package
    pub validation_package: Option<Arc<ValidationPackage>>,
}

#[derive(Clone, Constructor)]
pub struct ValidateHostAccess {
    pub workspace: CallZomeWorkspaceLock,
    pub network: HolochainP2pCell,
}

impl From<ValidateHostAccess> for HostAccess {
    fn from(validate_host_access: ValidateHostAccess) -> Self {
        Self::Validate(validate_host_access)
    }
}

impl From<&ValidateHostAccess> for HostFnAccess {
    fn from(_: &ValidateHostAccess) -> Self {
        let mut access = Self::none();
        access.read_workspace = Permission::Allow;
        access
    }
}

impl Invocation for ValidateInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        // entries are specific to zomes so only validate in the zome the entry is defined in
        // note that here it is possible there is a zome/entry mismatch
        // we rely on the invocation to be built correctly
        ZomesToInvoke::One(self.zome_name.clone())
    }
    fn fn_components(&self) -> FnComponents {
        let mut fns = vec!["validate".into()];
        match self.element.entry().as_option() {
            Some(Entry::Agent(_)) => fns.push("agent".into()),
            Some(Entry::App(_)) => fns.push("entry".into()),
            Some(Entry::CapClaim(_)) => fns.push("cap_claim".into()),
            Some(Entry::CapGrant(_)) => fns.push("cap_grant".into()),
            None => match self.element.header() {
                Header::ElementDelete(_) => fns.push("delete".into()),
                Header::EntryUpdate(_) => fns.push("update".into()),
                Header::LinkRemove(_) => fns.push("remove_link".into()),
                _ => (),
            },
        }
        fns.into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(ValidateData::from(self).try_into()?))
    }
}

// TODO: I think this is wrong and unused?
// impl TryFrom<ValidateInvocation> for HostInput {
//     type Error = SerializedBytesError;
//     fn try_from(validate_invocation: ValidateInvocation) -> Result<Self, Self::Error> {
//         Ok(Self::new((&*validate_invocation.element).try_into()?))
//     }
// }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<Vec<(ZomeName, ValidateCallbackResult)>> for ValidateResult {
    fn from(a: Vec<(ZomeName, ValidateCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
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
                    _ => Self::UnresolvedDependencies(ud),
                },
                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,
            }
        })
    }
}

impl From<ValidateInvocation> for ValidateData {
    fn from(vi: ValidateInvocation) -> Self {
        Self {
            element: Element::clone(&vi.element),
            validation_package: vi
                .validation_package
                .map(|vp| ValidationPackage::clone(&vp)),
        }
    }
}

#[cfg(test)]
mod test {

    use super::ValidateResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidateHostAccessFixturator;
    use crate::fixt::ValidateInvocationFixturator;
    use crate::fixt::ZomeCallCapGrantFixturator;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::{dna::zome::HostFnAccess, dna::zome::Permission, fixt::*};
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::validate::ValidateCallbackResult;
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
        let validate_host_access = ValidateHostAccessFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let mut access = HostFnAccess::none();
        access.read_workspace = Permission::Allow;
        assert_eq!(HostFnAccess::from(&validate_host_access), access);
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
        let el = fixt!(Element, agent_entry);
        validate_invocation.element = Arc::new(el);
        let mut expected = vec!["validate", "validate_agent"];
        for fn_component in validate_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }

        let agent_entry = Entry::App(
            AppEntryBytesFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap()
                .into(),
        );
        let el = fixt!(Element, agent_entry);
        validate_invocation.element = Arc::new(el);
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
        let el = fixt!(Element, agent_entry);
        validate_invocation.element = Arc::new(el);
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
        let el = fixt!(Element, agent_entry);
        validate_invocation.element = Arc::new(el);
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
            HostInput::new(SerializedBytes::try_from(&*validate_invocation.element).unwrap()),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {

    use super::ValidateResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::state::source_chain::SourceChainResult;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_types::fixt::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::CommitEntryOutput;
    use holochain_zome_types::Entry;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_valid() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateValid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateValid.into();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_invalid() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateInvalid.into();

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_multi() {
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
        let el = fixt!(Element, entry);
        validate_invocation.element = Arc::new(el);

        let result = ribosome
            .run_validate(fixt!(ValidateHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateResult::Invalid("esoteric edge case".into()));
    }

    #[tokio::test(threaded_scheduler)]
    async fn pass_validate_test<'a>() {
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        let output: CommitEntryOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Validate, "always_validates", ());

        // the chain head should be the committed entry header
        let chain_head = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            SourceChainResult::Ok(
                workspace_lock
                    .read()
                    .await
                    .source_chain
                    .chain_head()?
                    .to_owned(),
            )
        })
        .unwrap();

        assert_eq!(chain_head, output.into_inner(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn fail_validate_test<'a>() {
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        // commits fail validation if we don't do genesis
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock.clone();

        let output: CommitEntryOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Validate, "never_validates", ());

        // the chain head should be the committed entry header
        let chain_head = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            SourceChainResult::Ok(
                workspace_lock
                    .read()
                    .await
                    .source_chain
                    .chain_head()?
                    .to_owned(),
            )
        })
        .unwrap();

        assert_eq!(chain_head, output.into_inner(),);
    }
}
