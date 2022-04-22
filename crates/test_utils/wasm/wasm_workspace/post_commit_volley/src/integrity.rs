use holochain_deterministic_integrity::prelude::*;

pub const PINGS: usize = 5;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Ping(pub AgentPubKey);

#[hdk_entry_defs]
pub enum EntryTypes {
    Ping(Ping),
}
