use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);

#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_defs]
pub enum EntryTypes {
    #[entry_def(required_validations = 5)]
    Post(Post),
    #[entry_def(required_validations = 5)]
    Msg(Msg),
}

pub fn post() -> EntryTypes {
    EntryTypes::Post(Post("foo".into()))
}

pub fn msg() -> EntryTypes {
    EntryTypes::Msg(Msg("hi".into()))
}
