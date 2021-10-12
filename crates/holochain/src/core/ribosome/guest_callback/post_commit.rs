use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome: Zome,
    headers: HeaderHashes,
}

impl PostCommitInvocation {
    pub fn new(zome: Zome, headers: HeaderHashes) -> Self {
        Self { zome, headers }
    }
}

#[derive(Clone, Constructor)]
pub struct PostCommitHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: HolochainP2pCell,
}

impl From<PostCommitHostAccess> for HostContext {
    fn from(post_commit_host_access: PostCommitHostAccess) -> Self {
        Self::PostCommit(post_commit_host_access)
    }
}

impl From<&PostCommitHostAccess> for HostFnAccess {
    fn from(_: &PostCommitHostAccess) -> Self {
        Self::all()
    }
}

impl Invocation for PostCommitInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome.to_owned())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.headers)
    }
}

impl TryFrom<PostCommitInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        ExternIO::encode(&post_commit_invocation.headers)
    }
}

#[derive(PartialEq, Debug)]
pub enum PostCommitResult {
    Success,
    Fail(HeaderHashes, String),
}

impl From<Vec<(ZomeName, PostCommitCallbackResult)>> for PostCommitResult {
    fn from(a: Vec<(ZomeName, PostCommitCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
}

impl From<Vec<PostCommitCallbackResult>> for PostCommitResult {
    fn from(callback_results: Vec<PostCommitCallbackResult>) -> Self {
        // this is an optional callback so defaults to success
        callback_results.into_iter().fold(Self::Success, |acc, x| {
            match x {
                // fail overrides everything
                PostCommitCallbackResult::Fail(header_hashes, fail_string) => {
                    Self::Fail(header_hashes, fail_string)
                }
                // success allows acc to continue
                PostCommitCallbackResult::Success => acc,
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::PostCommitResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::HeaderHashesFixturator;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use ::fixt::prelude::*;
    use holochain_types::prelude::*;
    use holochain_zome_types::post_commit::PostCommitCallbackResult;
    use holochain_zome_types::ExternIO;

    #[test]
    fn post_commit_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_success = || PostCommitResult::Success;
        let result_fail = || {
            PostCommitResult::Fail(
                HeaderHashesFixturator::new(::fixt::Empty).next().unwrap(),
                StringFixturator::new(::fixt::Empty).next().unwrap(),
            )
        };

        let cb_success = || PostCommitCallbackResult::Success;
        let cb_fail = || {
            PostCommitCallbackResult::Fail(
                HeaderHashesFixturator::new(::fixt::Empty).next().unwrap(),
                StringFixturator::new(::fixt::Empty).next().unwrap(),
            )
        };

        for (mut results, expected) in vec![
            (vec![], result_success()),
            (vec![cb_success()], result_success()),
            (vec![cb_fail()], result_fail()),
            (vec![cb_fail(), cb_success()], result_fail()),
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
    async fn post_commit_invocation_access() {
        let post_commit_host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(
            HostFnAccess::from(&post_commit_host_access),
            HostFnAccess::all()
        );
    }

    #[test]
    fn post_commit_invocation_zomes() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let zome = post_commit_invocation.zome.clone();
        assert_eq!(ZomesToInvoke::One(zome), post_commit_invocation.zomes(),);
    }

    #[test]
    fn post_commit_invocation_fn_components() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["post_commit"];
        for fn_component in post_commit_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn post_commit_invocation_host_input() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let host_input = post_commit_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(HeaderHashesFixturator::new(::fixt::Empty).next().unwrap()).unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::PostCommitResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::PostCommitHostAccessFixturator;
    use crate::fixt::PostCommitInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use holo_hash::fixt::HeaderHashFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_unimplemented() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = TestWasm::Foo.into();

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, PostCommitResult::Success,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_implemented_success() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitSuccess]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = TestWasm::PostCommitSuccess.into();

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(result, PostCommitResult::Success,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_post_commit_implemented_fail() {
        let host_access = PostCommitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitFail]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome = TestWasm::PostCommitFail.into();

        let result = ribosome
            .run_post_commit(host_access, post_commit_invocation)
            .unwrap();
        assert_eq!(
            result,
            PostCommitResult::Fail(
                vec![HeaderHashFixturator::new(::fixt::Empty)
                    .next()
                    .unwrap()
                    .into()]
                .into(),
                "empty header fail".into()
            ),
        );
    }
}
