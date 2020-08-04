use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::zome::HostFnAccess;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::header::LinkAdd;
use holochain_zome_types::validate_link_add::ValidateLinkAddCallbackResult;
use holochain_zome_types::validate_link_add::ValidateLinkAddData;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::sync::Arc;

#[derive(Clone)]
pub struct ValidateLinkAddInvocation {
    pub zome_name: ZomeName,
    // Arc here as LinkAdd contains arbitrary bytes in the tag
    pub link_add: Arc<LinkAdd>,
    pub base: Arc<Entry>,
    pub target: Arc<Entry>,
}

impl ValidateLinkAddInvocation {
    pub fn new(zome_name: ZomeName, link_add: LinkAdd, base: Entry, target: Entry) -> Self {
        Self {
            zome_name,
            link_add: Arc::new(link_add),
            base: Arc::new(base),
            target: Arc::new(target),
        }
    }
}

impl From<ValidateLinkAddInvocation> for ValidateLinkAddData {
    fn from(validate_link_add_invocation: ValidateLinkAddInvocation) -> Self {
        Self {
            link_add: (*validate_link_add_invocation.link_add).clone(),
            base: (*validate_link_add_invocation.base).clone(),
            target: (*validate_link_add_invocation.target).clone(),
        }
    }
}

#[derive(Clone, Constructor)]
pub struct ValidateLinkAddHostAccess;

impl From<ValidateLinkAddHostAccess> for HostAccess {
    fn from(validate_link_add_host_access: ValidateLinkAddHostAccess) -> Self {
        Self::ValidateLinkAdd(validate_link_add_host_access)
    }
}

impl From<&ValidateLinkAddHostAccess> for HostFnAccess {
    fn from(_: &ValidateLinkAddHostAccess) -> Self {
        Self::none()
    }
}

impl Invocation for ValidateLinkAddInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        // links are specific to zomes so only validate in the zome the link is defined in
        // note that here it is possible there is a zome/link mismatch
        // we rely on the invocation to be built correctly
        ZomesToInvoke::One(self.zome_name.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "validate_link".into(),
            // "add" is optional, validate_link is fine too
            "add".into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(ValidateLinkAddData::from(self).try_into()?))
    }
}

impl TryFrom<ValidateLinkAddInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(
        validate_link_add_invocation: ValidateLinkAddInvocation,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&*validate_link_add_invocation.link_add).try_into()?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateLinkAddResult {
    Valid,
    Invalid(String),
}

impl From<Vec<(ZomeName, ValidateLinkAddCallbackResult)>> for ValidateLinkAddResult {
    fn from(a: Vec<(ZomeName, ValidateLinkAddCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateLinkAddCallbackResult>> for ValidateLinkAddResult {
    fn from(callback_results: Vec<ValidateLinkAddCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateLinkAddCallbackResult::Invalid(i) => Self::Invalid(i),
                // valid x allows validation to continue
                ValidateLinkAddCallbackResult::Valid => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {

    use super::ValidateLinkAddResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidateHostAccessFixturator;
    use crate::fixt::ValidateInvocationFixturator;
    use crate::fixt::ZomeCallCapGrantFixturator;
    use ::fixt::prelude::*;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::{dna::zome::HostFnAccess, fixt::CapClaimFixturator};
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::validate_link_add::ValidateLinkAddCallbackResult;
    use holochain_zome_types::HostInput;
    use rand::seq::SliceRandom;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn validate_callback_result_fold() {
        let mut rng = thread_rng();

        let result_valid = || ValidateLinkAddResult::Valid;
        // let result_ud = || ValidateLinkAddResult::UnresolvedDependencies(vec![]);
        let result_invalid = || ValidateLinkAddResult::Invalid("".into());

        let cb_valid = || ValidateLinkAddCallbackResult::Valid;
        // let cb_ud = || ValidateCallbackResult::UnresolvedDependencies(vec![]);
        let cb_invalid = || ValidateLinkAddCallbackResult::Invalid("".into());

        for (mut results, expected) in vec![
            (vec![], result_valid()),
            (vec![cb_valid()], result_valid()),
            (vec![cb_invalid()], result_invalid()),
            // (vec![cb_ud()], result_ud()),
            (vec![cb_invalid(), cb_valid()], result_invalid()),
            // (vec![cb_invalid(), cb_ud()], result_invalid()),
            // (vec![cb_valid(), cb_ud()], result_ud()),
            // (vec![cb_valid(), cb_ud(), cb_invalid()], result_invalid()),
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
        assert_eq!(
            HostFnAccess::from(&validate_host_access),
            HostFnAccess::none(),
        );
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
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {

    use super::ValidateLinkAddHostAccess;
    use super::ValidateLinkAddResult;
    use crate::core::ribosome::RibosomeT;
    // use crate::core::state::source_chain::SourceChainResult;
    // use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::curve::Zomes;
    use crate::fixt::ValidateLinkAddInvocationFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    // use crate::fixt::ZomeCallHostAccessFixturator;
    // use fixt::prelude::*;
    // use futures::future::BoxFuture;
    // use futures::future::FutureExt;
    // use holo_hash::fixt::AgentPubKeyFixturator;
    // use holo_hash::HeaderHash;
    use holochain_wasm_test_utils::TestWasm;
    // use holochain_zome_types::CommitEntryOutput;
    // use holochain_zome_types::Entry;
    // use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_link_add_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateLinkAddInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_validate_link_add(ValidateLinkAddHostAccess, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateLinkAddResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_valid() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateLinkAddValid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateLinkAddInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateLinkAddValid.into();

        let result = ribosome
            .run_validate_link_add(ValidateLinkAddHostAccess, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateLinkAddResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_link_add_implemented_invalid() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateLinkAddInvalid]))
            .next()
            .unwrap();
        let mut validate_link_add_invocation =
            ValidateLinkAddInvocationFixturator::new(fixt::Empty)
                .next()
                .unwrap();
        validate_link_add_invocation.zome_name = TestWasm::ValidateLinkAddInvalid.into();

        let result = ribosome
            .run_validate_link_add(ValidateLinkAddHostAccess, validate_link_add_invocation)
            .unwrap();
        assert_eq!(
            result,
            ValidateLinkAddResult::Invalid("esoteric edge case (link version)".into()),
        );
    }
    //
    // #[tokio::test(threaded_scheduler)]
    // async fn test_validate_implemented_multi() {
    //     let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateInvalid]))
    //         .next()
    //         .unwrap();
    //     let mut validate_invocation = ValidateLinkAddInvocationFixturator::new(fixt::Empty)
    //         .next()
    //         .unwrap();
    //     let entry = Entry::Agent(
    //         AgentPubKeyFixturator::new(fixt::Unpredictable)
    //             .next()
    //             .unwrap()
    //             .into(),
    //     );
    //
    //     validate_invocation.zome_name = TestWasm::ValidateInvalid.into();
    //     validate_invocation.link_add = Arc::new(link_add);
    //
    //     let result = ribosome
    //         .run_validate_link_add(ValidateLinkAddHostAccess, validate_invocation)
    //         .unwrap();
    //     assert_eq!(result, ValidateLinkAddResult::Invalid("esoteric edge case".into()));
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // async fn pass_validate_test<'a>() {
    //     // test workspace boilerplate
    //     let env = holochain_state::test_utils::test_cell_env();
    //     let dbs = env.dbs().await;
    //     let env_ref = env.guard().await;
    //     let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
    //     let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();
    //
    //     // commits fail validation if we don't do genesis
    //     crate::core::workflow::fake_genesis(&mut workspace.source_chain)
    //         .await
    //         .unwrap();
    //
    //     let (_g, raw_workspace) =
    //         crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
    //             &mut workspace,
    //         );
    //     let mut host_access = fixt!(ZomeCallHostAccess);
    //     host_access.workspace = raw_workspace.clone();
    //
    //     let output: CommitEntryOutput =
    //         crate::call_test_ribosome!(host_access, TestWasm::Validate, "always_validates", ());
    //
    //     // the chain head should be the committed entry header
    //     let call =
    //         |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
    //             async move {
    //                 let source_chain = &mut workspace.source_chain;
    //                 Ok(source_chain.chain_head()?.to_owned())
    //             }
    //             .boxed()
    //         };
    //     let chain_head =
    //         tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
    //             unsafe { raw_workspace.apply_mut(call).await }
    //         }))
    //         .unwrap()
    //         .unwrap()
    //         .unwrap();
    //
    //     assert_eq!(chain_head, output.into_inner(),);
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // async fn fail_validate_test<'a>() {
    //     // test workspace boilerplate
    //     let env = holochain_state::test_utils::test_cell_env();
    //     let dbs = env.dbs().await;
    //     let env_ref = env.guard().await;
    //     let reader = holochain_state::env::ReadManager::reader(&env_ref).unwrap();
    //     let mut workspace = <crate::core::workflow::call_zome_workflow::CallZomeWorkspace as crate::core::state::workspace::Workspace>::new(&reader, &dbs).unwrap();
    //
    //     // commits fail validation if we don't do genesis
    //     crate::core::workflow::fake_genesis(&mut workspace.source_chain)
    //         .await
    //         .unwrap();
    //
    //     let (_g, raw_workspace) =
    //         crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
    //             &mut workspace,
    //         );
    //
    //     let mut host_access = fixt!(ZomeCallHostAccess);
    //     host_access.workspace = raw_workspace.clone();
    //
    //     let output: CommitEntryOutput =
    //         crate::call_test_ribosome!(host_access, TestWasm::Validate, "never_validates", ());
    //
    //     // the chain head should be the committed entry header
    //     let call =
    //         |workspace: &'a mut CallZomeWorkspace| -> BoxFuture<'a, SourceChainResult<HeaderHash>> {
    //             async move {
    //                 let source_chain = &mut workspace.source_chain;
    //                 Ok(source_chain.chain_head()?.to_owned())
    //             }
    //             .boxed()
    //         };
    //     let chain_head =
    //         tokio_safe_block_on::tokio_safe_block_forever_on(tokio::task::spawn(async move {
    //             unsafe { raw_workspace.apply_mut(call).await }
    //         }))
    //         .unwrap()
    //         .unwrap()
    //         .unwrap();
    //
    //     assert_eq!(chain_head, output.into_inner(),);
    // }
}
