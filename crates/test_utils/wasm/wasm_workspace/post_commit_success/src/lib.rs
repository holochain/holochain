use hdk3::prelude::*;

#[hdk(extern)]
fn post_commit(_: HeaderHashes) -> ExternResult<PostCommitCallbackResult> {
    Ok(PostCommitCallbackResult::Success)
}
