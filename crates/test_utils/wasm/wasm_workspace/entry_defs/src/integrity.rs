use hdi::prelude::*;

#[hdi_entry_helper]
pub struct Post;

#[hdi_entry_helper]
pub struct Comment;

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Post(Post),
    #[entry_type(visibility = "private")]
    Comment(Comment),
}
