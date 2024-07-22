use derive_more::*;
use hdi::prelude::*;

#[hdi_entry_helper]
#[derive(Constructor)]
pub struct Channel {
    pub name: String,
}

#[hdi_entry_helper]
#[derive(Constructor)]
pub struct ChannelMessage {
    pub message: String,
}

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Channel(Channel),
    ChannelMessage(ChannelMessage),
}

#[hdi_link_types]
pub enum LinkTypes {
    Any,
    Path,
}
