use hdk3::prelude::*;

#[hdk(extern)]
fn post_commit(_: ()) -> ExternResult<PostCommitCallbackResult> {
    Ok(PostCommitCallbackResult::Fail(
        vec![HeaderHash::from_raw_bytes(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0x99, 0xf6, 0x1f, 0xc2,
        ])]
        .into(),
        "empty header fail".into(),
    ))
}
