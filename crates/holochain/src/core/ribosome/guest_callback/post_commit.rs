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

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome_name: ZomeName,
    headers: HeaderHashes,
}

fixturator!(
    PostCommitInvocation,
    {
        let post_commit_invocation = PostCommitInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            headers: HeaderHashesFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        post_commit_invocation
    },
    {
        let post_commit_invocation = PostCommitInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            headers: HeaderHashesFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        post_commit_invocation
    },
    {
        let post_commit_invocation = PostCommitInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            headers: HeaderHashesFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        post_commit_invocation
    }
);

impl Invocation for PostCommitInvocation {
    fn allow_side_effects(&self) -> bool {
        true
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
mod test {

    use super::PostCommitInvocationFixturator;
    use super::PostCommitResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holo_hash::HeaderHashFixturator;
    use holochain_wasm_test_utils::TestWasm;

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
