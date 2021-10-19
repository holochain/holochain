use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holo_hash::AnyDhtHash;
use holochain_p2p::HolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspaceReadOnly;
use holochain_types::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct ValidateLinkInvocation<I>
where
    I: Invocation,
{
    invocation: I,
}

impl ValidateLinkInvocation<ValidateCreateLinkInvocation> {
    pub fn new(invocation: ValidateCreateLinkInvocation) -> Self {
        Self { invocation }
    }
}

impl ValidateLinkInvocation<ValidateDeleteLinkInvocation> {
    pub fn new(invocation: ValidateDeleteLinkInvocation) -> Self {
        Self { invocation }
    }
}

#[derive(Clone)]
pub struct ValidateCreateLinkInvocation {
    pub zome: Zome,
    // Arc here as CreateLink contains arbitrary bytes in the tag
    pub link_add: Arc<CreateLink>,
    pub base: Arc<Entry>,
    pub target: Arc<Entry>,
}

#[derive(Clone, derive_more::Constructor)]
pub struct ValidateDeleteLinkInvocation {
    pub zome: Zome,
    pub delete_link: DeleteLink,
}

impl ValidateCreateLinkInvocation {
    pub fn new(zome: Zome, link_add: CreateLink, base: Entry, target: Entry) -> Self {
        Self {
            zome,
            link_add: Arc::new(link_add),
            base: Arc::new(base),
            target: Arc::new(target),
        }
    }
}

impl From<ValidateCreateLinkInvocation> for ValidateCreateLinkData {
    fn from(validate_create_link_invocation: ValidateCreateLinkInvocation) -> Self {
        Self {
            link_add: (*validate_create_link_invocation.link_add).clone(),
            base: (*validate_create_link_invocation.base).clone(),
            target: (*validate_create_link_invocation.target).clone(),
        }
    }
}

impl From<ValidateDeleteLinkInvocation> for ValidateDeleteLinkData {
    fn from(validate_delete_link_invocation: ValidateDeleteLinkInvocation) -> Self {
        Self {
            delete_link: validate_delete_link_invocation.delete_link,
        }
    }
}
#[derive(Clone, Constructor)]
pub struct ValidateLinkHostAccess {
    pub workspace: HostFnWorkspaceReadOnly,
    pub network: HolochainP2pDna,
}

impl From<ValidateLinkHostAccess> for HostContext {
    fn from(validate_link_add_host_access: ValidateLinkHostAccess) -> Self {
        Self::ValidateCreateLink(validate_link_add_host_access)
    }
}

impl From<&ValidateLinkHostAccess> for HostFnAccess {
    fn from(_: &ValidateLinkHostAccess) -> Self {
        let mut access = Self::none();
        access.keystore_deterministic = Permission::Allow;
        access.read_workspace_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        access
    }
}

impl<I> Invocation for ValidateLinkInvocation<I>
where
    I: Invocation,
{
    fn zomes(&self) -> ZomesToInvoke {
        self.invocation.zomes()
    }
    fn fn_components(&self) -> FnComponents {
        self.invocation.fn_components()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        self.invocation.host_input()
    }
}

impl Invocation for ValidateCreateLinkInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        // links are specific to zomes so only validate in the zome the link is defined in
        // note that here it is possible there is a zome/link mismatch
        // we rely on the invocation to be built correctly
        ZomesToInvoke::One(self.zome.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["validate_create_link".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(ValidateCreateLinkData::from(self))
    }
}

impl Invocation for ValidateDeleteLinkInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        // links are specific to zomes so only validate in the zome the link is defined in
        // note that here it is possible there is a zome/link mismatch
        // we rely on the invocation to be built correctly
        ZomesToInvoke::One(self.zome.clone())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["validate_delete_link".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(ValidateDeleteLinkData::from(self))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateLinkResult {
    Valid,
    Invalid(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl From<Vec<(ZomeName, ValidateLinkCallbackResult)>> for ValidateLinkResult {
    fn from(a: Vec<(ZomeName, ValidateLinkCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<ValidateLinkCallbackResult>> for ValidateLinkResult {
    fn from(callback_results: Vec<ValidateLinkCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateLinkCallbackResult::Invalid(i) => Self::Invalid(i),
                // return unresolved dependencies if it's otherwise valid
                ValidateLinkCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud),
                },
                // valid x allows validation to continue
                ValidateLinkCallbackResult::Valid => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::ValidateLinkResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holochain_types::access::Permission;
    use holochain_types::prelude::*;
    use holochain_zome_types::validate_link::ValidateCreateLinkData;
    use holochain_zome_types::validate_link::ValidateLinkCallbackResult;
    use holochain_zome_types::ExternIO;
    use rand::seq::SliceRandom;

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_link_add_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_valid = || ValidateLinkResult::Valid;
        let result_invalid = || ValidateLinkResult::Invalid("".into());

        let cb_valid = || ValidateLinkCallbackResult::Valid;
        let cb_invalid = || ValidateLinkCallbackResult::Invalid("".into());

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

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_link_add_invocation_allow_side_effects() {
        let validate_link_add_host_access =
            ValidateLinkHostAccessFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();
        let mut access = HostFnAccess::none();
        access.read_workspace_deterministic = Permission::Allow;
        access.bindings_deterministic = Permission::Allow;
        access.keystore_deterministic = Permission::Allow;
        assert_eq!(HostFnAccess::from(&validate_link_add_host_access), access,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_link_add_invocation_zomes() {
        let validate_create_link_invocation =
            ValidateCreateLinkInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();
        let zome = validate_create_link_invocation.zome.clone();
        assert_eq!(
            ZomesToInvoke::One(zome),
            validate_create_link_invocation.zomes(),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_link_add_invocation_fn_components() {
        let validate_create_link_invocation =
            ValidateCreateLinkInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();

        let mut expected = vec!["validate_create_link"];
        for fn_component in validate_create_link_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validate_link_add_invocation_host_input() {
        let validate_create_link_invocation =
            ValidateCreateLinkInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();

        let host_input = validate_create_link_invocation
            .clone()
            .host_input()
            .unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(&ValidateCreateLinkData::from(
                validate_create_link_invocation
            ))
            .unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::ValidateLinkResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use holo_hash::HeaderHash;
    use holochain_state::source_chain::SourceChainResult;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_link_add_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let validate_invocation =
            ValidateLinkInvocationCreateFixturator::new(Zome::from(TestWasm::Foo))
                .next()
                .unwrap();

        let result = ribosome
            .run_validate_link(fixt!(ValidateLinkHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateLinkResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_implemented_valid() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateCreateLinkValid]))
            .next()
            .unwrap();
        let validate_invocation = ValidateLinkInvocationCreateFixturator::new(Zome::from(
            TestWasm::ValidateCreateLinkValid,
        ))
        .next()
        .unwrap();

        let result = ribosome
            .run_validate_link(fixt!(ValidateLinkHostAccess), validate_invocation)
            .unwrap();
        assert_eq!(result, ValidateLinkResult::Valid,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_link_add_implemented_invalid() {
        let ribosome =
            RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidateCreateLinkInvalid]))
                .next()
                .unwrap();
        let validate_create_link_invocation = ValidateLinkInvocationCreateFixturator::new(
            Zome::from(TestWasm::ValidateCreateLinkInvalid),
        )
        .next()
        .unwrap();

        let result = ribosome
            .run_validate_link(
                fixt!(ValidateLinkHostAccess),
                validate_create_link_invocation,
            )
            .unwrap();
        assert_eq!(
            result,
            ValidateLinkResult::Invalid("esoteric edge case (link version)".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pass_validate_link_add_test<'a>() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::ValidateLink, "add_valid_link", ())
                .unwrap();

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            SourceChainResult::Ok(host_access.workspace.source_chain().chain_head()?.0)
        })
        .unwrap();

        assert_eq!(chain_head, output,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fail_validate_link_add_test<'a>() {
        let host_access = fixt!(ZomeCallHostAccess, Predictable);

        let output: HeaderHash =
            crate::call_test_ribosome!(host_access, TestWasm::ValidateLink, "add_invalid_link", ())
                .unwrap();

        // the chain head should be the committed entry header
        let chain_head = tokio_helper::block_forever_on(async move {
            SourceChainResult::Ok(host_access.workspace.source_chain().chain_head()?.0)
        })
        .unwrap();

        assert_eq!(chain_head, output,);
    }
}
