use crate::hdk3::prelude::*;

#[hdk_entry(id = "post")]
struct Post;

#[hdk_entry(id = "comment", visibility = "private")]
struct Comment;

entry_defs![Post::entry_def(), Comment::entry_def()];
