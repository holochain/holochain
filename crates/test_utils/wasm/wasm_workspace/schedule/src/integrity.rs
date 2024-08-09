use hdi::prelude::*;

pub const TICKS: usize = 5;

#[hdi_entry_helper]
pub struct TickInit;

#[hdi_entry_helper]
pub struct TockInit;

#[hdi_entry_helper]
pub struct Tick;

#[hdi_entry_helper]
pub struct Tock;

#[hdi_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    TickInit(TickInit),
    TockInit(TockInit),
    Tick(Tick),
    Tock(Tock),
}
