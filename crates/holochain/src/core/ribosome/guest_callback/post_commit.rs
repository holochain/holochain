use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::HeaderHashesFixturator;
use crate::fixt::ZomeNameFixturator;
use fixt::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::header::HeaderHashes;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use holochain_types::dna::zome::HostFnAccess;

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome_name: ZomeName,
    headers: HeaderHashes,
}

impl PostCommitInvocation {
    pub fn new(zome_name: ZomeName, headers: HeaderHashes) -> Self {
        Self { zome_name, headers }
    }
}

fixturator!(
    PostCommitInvocation;
    constructor fn new(ZomeName, HeaderHashes);
);

impl Invocation for PostCommitInvocation {
    fn allowed_access(&self) -> HostFnAccess {
        HostFnAccess::all()
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome_name.to_owned())
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.headers).try_into()?))
    }
}

impl TryFrom<PostCommitInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&post_commit_invocation.headers).try_into()?))
    }
}

#[derive(PartialEq, Debug)]
pub enum PostCommitResult {
    Success,
    Fail(HeaderHashes, String),
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
#[cfg(feature = "slow_tests")]
mod test {

    use super::PostCommitInvocationFixturator;
    use super::PostCommitResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::HeaderHashesFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use fixt::prelude::*;
    use holo_hash::HeaderHashFixturator;
    use holochain_serialized_bytes::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::post_commit::PostCommitCallbackResult;
    use holochain_zome_types::HostInput;

    #[tokio::test(threaded_scheduler)]
    async fn post_commit_callback_result_fold() {
        let mut rng = thread_rng();

        let result_success = || PostCommitResult::Success;
        let result_fail = || {
            PostCommitResult::Fail(
                HeaderHashesFixturator::new(fixt::Empty).next().unwrap(),
                StringFixturator::new(fixt::Empty).next().unwrap(),
            )
        };

        let cb_success = || PostCommitCallbackResult::Success;
        let cb_fail = || {
            PostCommitCallbackResult::Fail(
                HeaderHashesFixturator::new(fixt::Empty).next().unwrap(),
                StringFixturator::new(fixt::Empty).next().unwrap(),
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

    #[tokio::test(threaded_scheduler)]
    async fn post_commit_invocation_allow_side_effects() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert!(post_commit_invocation.allow_side_effects());
    }

    #[tokio::test(threaded_scheduler)]
    async fn post_commit_invocation_zomes() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let zome_name = post_commit_invocation.zome_name.clone();
        assert_eq!(
            ZomesToInvoke::One(zome_name),
            post_commit_invocation.zomes(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn post_commit_invocation_fn_components() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["post_commit"];
        for fn_component in post_commit_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn post_commit_invocation_host_input() {
        let post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();

        let host_input = post_commit_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(
                SerializedBytes::try_from(HeaderHashesFixturator::new(fixt::Empty).next().unwrap())
                    .unwrap()
            ),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_post_commit_unimplemented() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_post_commit(workspace, post_commit_invocation)
            .unwrap();
        assert_eq!(result, PostCommitResult::Success,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_post_commit_implemented_success() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitSuccess]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome_name = TestWasm::PostCommitSuccess.into();

        let result = ribosome
            .run_post_commit(workspace, post_commit_invocation)
            .unwrap();
        assert_eq!(result, PostCommitResult::Success,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_post_commit_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::PostCommitFail]))
            .next()
            .unwrap();
        let mut post_commit_invocation = PostCommitInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();
        post_commit_invocation.zome_name = TestWasm::PostCommitFail.into();

        let result = ribosome
            .run_post_commit(workspace, post_commit_invocation)
            .unwrap();
        assert_eq!(
            result,
            PostCommitResult::Fail(
                vec![HeaderHashFixturator::new(fixt::Empty)
                    .next()
                    .unwrap()
                    .into()]
                .into(),
                "empty header fail".into()
            ),
        );
    }
}
