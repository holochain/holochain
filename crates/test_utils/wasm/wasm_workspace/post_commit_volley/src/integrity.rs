use hdi::prelude::*;

pub const PINGS: usize = 5;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Ping(pub AgentPubKey);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Ping(Ping),
}
