use hdk3::prelude::*;

holochain_externs!();

const POST_ID: &str = "post";
struct Post;

const COMMENT_ID: &str = "comment";
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

entry_defs!(vec![Post::entry_def(), Comment::entry_def()]);
