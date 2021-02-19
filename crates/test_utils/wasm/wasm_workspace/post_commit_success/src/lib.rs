use hdk::prelude::*;

#[hdk_extern]
fn post_commit(_: HeaderHashes) -> ExternResult<PostCommitCallbackResult> {
    Ok(PostCommitCallbackResult::Success)
}
