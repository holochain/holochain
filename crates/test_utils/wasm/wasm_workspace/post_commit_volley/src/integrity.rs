use hdi::prelude::*;

pub const PINGS: usize = 5;

#[hdi_entry_helper]
#[derive(Clone)]
pub struct Ping(pub AgentPubKey);

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Ping(Ping),
}
