use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post;

#[hdk_entry_helper]
pub struct Comment;

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Post),
    #[entry_def(visibility = "private")]
    Comment(Comment),
}
