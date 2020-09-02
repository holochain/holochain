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
    use crate::fixt::ValidateLinkAddHostAccessFixturator;
    use crate::fixt::ValidateLinkAddInvocationFixturator;
    use ::fixt::prelude::*;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::dna::zome::HostFnAccess;
    use holochain_zome_types::validate_link_add::ValidateLinkAddCallbackResult;
    use holochain_zome_types::validate_link_add::ValidateLinkAddData;
    use holochain_zome_types::HostInput;
    use rand::seq::SliceRandom;

    #[tokio::test(threaded_scheduler)]
    async fn validate_link_add_callback_result_fold() {
        let mut rng = thread_rng();

        let result_valid = || ValidateLinkAddResult::Valid;
        let result_invalid = || ValidateLinkAddResult::Invalid("".into());

        let cb_valid = || ValidateLinkAddCallbackResult::Valid;
        let cb_invalid = || ValidateLinkAddCallbackResult::Invalid("".into());

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
            ValidateLinkAddHostAccessFixturator::new(fixt::Unpredictable)
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
            ValidateLinkAddInvocationFixturator::new(fixt::Unpredictable)
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
            ValidateLinkAddInvocationFixturator::new(fixt::Unpredictable)
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
            ValidateLinkAddInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();

        let host_input = validate_link_add_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(
                SerializedBytes::try_from(&ValidateLinkAddData::from(validate_link_add_invocation))
                    .unwrap()
            ),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {

    use super::ValidateLinkAddHostAccess;
    use super::ValidateLinkAddResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::state::source_chain::SourceChainResult;
    use crate::core::workflow::call_zome_workflow::CallZomeWorkspace;
    use crate::fixt::curve::Zomes;
    use crate::fixt::ValidateLinkAddInvocationFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_wasm_test_utils::TestWasm;

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

    #[tokio::test(threaded_scheduler)]
    async fn pass_validate_link_add_test<'a>() {
        // test workspace boilerplate
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let dbs = env.dbs();
        let mut workspace = CallZomeWorkspace::new(env.clone().into(), &dbs)
            .await
            .unwrap();

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
        let dbs = env.dbs();
        let mut workspace = CallZomeWorkspace::new(env.clone().into(), &dbs)
            .await
            .unwrap();

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
