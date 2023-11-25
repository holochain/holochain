use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post;

#[hdk_entry_helper]
pub struct Comment;

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Post),
    #[entry_type(visibility = "private")]
    Comment(Comment),
}
