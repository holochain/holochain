use hdk3::prelude::*;

#[hdk(extern)]
fn post_commit(_: ()) -> ExternResult<PostCommitCallbackResult> {
    Ok(PostCommitCallbackResult::Success)
}
