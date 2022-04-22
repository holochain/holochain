use derive_more::*;
use holochain_deterministic_integrity::prelude::*;

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
pub enum EntryTypes {
    Channel(Channel),
    ChannelMessage(ChannelMessage),
}

#[hdk_link_types]
pub enum LinkTypes {
    Any = HdkLinkType::Any as u8,
}