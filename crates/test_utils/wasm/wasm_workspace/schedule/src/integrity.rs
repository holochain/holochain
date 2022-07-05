use hdi::prelude::*;

pub const TICKS: usize = 5;

#[hdk_entry_helper]
pub struct Tick;

#[hdk_entry_helper]
pub struct Tock;

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Tick(Tick),
    Tock(Tock),
}
