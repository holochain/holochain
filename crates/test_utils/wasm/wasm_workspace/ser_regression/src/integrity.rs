use derive_more::*;
use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Constructor)]
pub struct Channel {
    pub name: String,
}

#[hdk_entry_helper]
#[derive(Constructor)]
pub struct ChannelMessage {
    pub message: String,
}

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Channel(Channel),
    ChannelMessage(ChannelMessage),
}

#[hdk_link_types]
pub enum LinkTypes {
    Any,
    Path,
}
