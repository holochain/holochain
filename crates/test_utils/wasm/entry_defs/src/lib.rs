use hdk3::prelude::*;

const POST_ID: &str = "post";
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Post;

const COMMENT_ID: &str = "comment";
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct Comment;

entry_def!(Post EntryDef {
    id: POST_ID.into(),
    ..Default::default()
});

entry_def!(Comment EntryDef {
    id: COMMENT_ID.into(),
    visibility: EntryVisibility::Private,
    ..Default::default()
});

entry_defs![Post::entry_def(), Comment::entry_def()];
