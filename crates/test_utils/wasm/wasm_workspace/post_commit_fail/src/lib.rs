use hdk3::prelude::*;

#[hdk_extern]
fn post_commit(_: HeaderHashes) -> ExternResult<PostCommitCallbackResult> {
    Ok(PostCommitCallbackResult::Fail(
        vec![HeaderHash::from_raw_36(vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0x99, 0xf6, 0x1f, 0xc2,
        ])]
        .into(),
        "empty header fail".into(),
    ))
}
