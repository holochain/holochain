use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::zome::HostFnAccess;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::header::CreateLink;
use holochain_zome_types::validate_link_add::ValidateCreateLinkCallbackResult;
use holochain_zome_types::validate_link_add::ValidateCreateLinkData;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::sync::Arc;

#[derive(Clone)]
pub struct ValidateCreateLinkInvocation {
    pub zome_name: ZomeName,
    // Arc here as CreateLink contains arbitrary bytes in the tag
    pub link_add: Arc<CreateLink>,
    pub base: Arc<Entry>,
    pub target: Arc<Entry>,
}

impl ValidateCreateLinkInvocation {
    pub fn new(zome_name: ZomeName, link_add: CreateLink, base: Entry, target: Entry) -> Self {
        Self {
            zome_name,
            link_add: Arc::new(link_add),
            base: Arc::new(base),
            target: Arc::new(target),
        }
    }
}

impl From<ValidateCreateLinkInvocation> for ValidateCreateLinkData {
    fn from(validate_link_add_invocation: ValidateCreateLinkInvocation) -> Self {
        Self {
            link_add: (*validate_link_add_invocation.link_add).clone(),
            base: (*validate_link_add_invocation.base).clone(),
            target: (*validate_link_add_invocation.target).clone(),
        }
    }
}

#[derive(Clone, Constructor)]
pub struct ValidateCreateLinkHostAccess;

impl From<ValidateCreateLinkHostAccess> for HostAccess {
    fn from(validate_link_add_host_access: ValidateCreateLinkHostAccess) -> Self {
        Self::ValidateCreateLink(validate_link_add_host_access)
    }
}

impl From<&ValidateCreateLinkHostAccess> for HostFnAccess {
    fn from(_: &ValidateCreateLinkHostAccess) -> Self {
        Self::none()
    }
}

impl Invocation for ValidateCreateLinkInvocation {
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
        Ok(HostInput::new(
            ValidateCreateLinkData::from(self).try_into()?,
        ))
    }
}

impl TryFrom<ValidateCreateLinkInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(
        validate_link_add_invocation: ValidateCreateLinkInvocation,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&*validate_link_add_invocation.link_add).try_into()?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCreateLinkResult {
    Valid,
    Invalid(String),
}

impl From<Vec<(ZomeName, ValidateCreateLinkCallbackResult)>> for ValidateCreateLinkResult {
    fn from(a: Vec<(ZomeName, ValidateCreateLinkCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateCreateLinkCallbackResult>> for ValidateCreateLinkResult {
    fn from(callback_results: Vec<ValidateCreateLinkCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCreateLinkCallbackResult::Invalid(i) => Self::Invalid(i),
                // valid x allows validation to continue
                ValidateCreateLinkCallbackResult::Valid => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {

    use super::ValidateCreateLinkResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidateCreateLinkHostAccessFixturator;
    use crate::fixt::ValidateCreateLinkInvocationFixturator;
    use ::fixt::prelude::*;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::dna::zome::HostFnAccess;
    use holochain_zome_types::validate_link_add::ValidateCreateLinkCallbackResult;
    use holochain_zome_types::validate_link_add::ValidateCreateLinkData;
    use holochain_zome_types::HostInput;
    use rand::seq::SliceRandom;

    #[tokio::test(threaded_scheduler)]
    async fn validate_link_add_callback_result_fold() {
        let mut rng = thread_rng();

        let result_valid = || ValidateCreateLinkResult::Valid;
        let result_invalid = || ValidateCreateLinkResult::Invalid("".into());

        let cb_valid = || ValidateCreateLinkCallbackResult::Valid;
        let cb_invalid = || ValidateCreateLinkCallbackResult::Invalid("".into());

        for (mut results, expected) in vec![
            (vec![], result_valid()),
            (vec![cb_valid()], result_valid()),
            (vec![cb_invalid()], result_invalid()),
            (vec![cb_invalid(), cb_valid()], result_invalid()),
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
    async fn validate_link_add_invocation_allow_side_effects() {
        let validate_link_add_host_access =
            ValidateCreateLinkHostAccessFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();
        assert_eq!(
            HostFnAccess::from(&validate_link_add_host_access),
            HostFnAccess::none(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_link_add_invocation_zomes() {
        let validate_link_add_invocation =
            ValidateCreateLinkInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();
        let zome_name = validate_link_add_invocation.zome_name.clone();
        assert_eq!(
            ZomesToInvoke::One(zome_name),
            validate_link_add_invocation.zomes(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_link_add_invocation_fn_components() {
        let validate_link_add_invocation =
            ValidateCreateLinkInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();

        let mut expected = vec!["validate_link", "validate_link_add"];
        for fn_component in validate_link_add_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn validate_link_add_invocation_host_input() {
        let validate_link_add_invocation =
            ValidateCreateLinkInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();

        let host_input = validate_link_add_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(
                SerializedBytes::try_from(&ValidateCreateLinkData::from(
                    validate_link_add_invocation
                ))
                .unwrap()
            ),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {

    use super::ValidateCreateLinkHostAccess;
    use super::ValidateCreateLinkResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::state::source_chain::SourceChainResult;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::curve::Zomes;
    use crate::fixt::ValidateCreateLinkInvocationFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_link_add_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateCreateLinkInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_validate_link_add(ValidateCreateLinkHostAccess, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateCreateLinkResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_implemented_valid() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateCreateLinkValid]))
            .next()
            .unwrap();
        let mut validate_invocation = ValidateCreateLinkInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        validate_invocation.zome_name = TestWasm::ValidateCreateLinkValid.into();

        let result = ribosome
            .run_validate_link_add(ValidateCreateLinkHostAccess, validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateCreateLinkResult::Valid,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validate_link_add_implemented_invalid() {
        let ribosome =
            WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateCreateLinkInvalid]))
                .next()
                .unwrap();
        let mut validate_link_add_invocation =
            ValidateCreateLinkInvocationFixturator::new(fixt::Empty)
                .next()
                .unwrap();
        validate_link_add_invocation.zome_name = TestWasm::ValidateCreateLinkInvalid.into();

        let result = ribosome
            .run_validate_link_add(ValidateCreateLinkHostAccess, validate_link_add_invocation)
            .unwrap();
        assert_eq!(
            result,
            ValidateCreateLinkResult::Invalid("esoteric edge case (link version)".into()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn pass_validate_link_add_test<'a>() {
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

        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::ValidateLink, "add_valid_link", ());

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

        assert_eq!(chain_head, output,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn fail_validate_link_add_test<'a>() {
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

        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::ValidateLink, "add_invalid_link", ());

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

        assert_eq!(chain_head, output,);
    }
}
