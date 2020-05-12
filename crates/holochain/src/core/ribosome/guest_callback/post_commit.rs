use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::header::HeaderHashes;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct PostCommitInvocation {
    // @todo PostCommitWorkspace?
    workspace: UnsafeInvokeZomeWorkspace,
    zome_name: ZomeName,
    headers: HeaderHashes,
}

impl Invocation for PostCommitInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        vec![self.zome_name.to_owned()]
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.headers).try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

impl TryFrom<PostCommitInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&post_commit_invocation.headers).try_into()?))
    }
}

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
