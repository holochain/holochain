use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Test(String);

#[hdk_link_types]
pub enum LinkTypes {
    SomeLinks,
    SomeOtherLinks,
}
