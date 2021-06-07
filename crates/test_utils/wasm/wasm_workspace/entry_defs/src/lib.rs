use hdk::prelude::*;

#[hdk_entry(id = "post")]
struct Post;

#[hdk_entry(id = "comment", visibility = "private")]
struct Comment;

pub struct Foo;

entry_defs![Post::entry_def(), Comment::entry_def()];

#[hdk_extern]
pub fn assert_indexes(_: ()) -> ExternResult<()> {
    assert_eq!(EntryDefIndex(0), entry_def_index!(Post)?);
    assert_eq!(EntryDefIndex(1), entry_def_index!(Comment)?);
    Ok(())
}
