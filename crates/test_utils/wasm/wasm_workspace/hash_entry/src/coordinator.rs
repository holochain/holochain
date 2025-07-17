use crate::integrity::*;
use hdk::prelude::*;
use EntryZomes::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityHashEntry(EntryTypes),
}

fn temperature() -> Temperature {
    Temperature(32, TemperatureUnit::Celcius)
}

#[hdk_extern]
fn twenty_three_degrees_entry_hash(_: ()) -> ExternResult<EntryHash> {
    let temp = temperature();
    let action_hash: ActionHash = create_entry(&IntegrityHashEntry(EntryTypes::Temperature(temp)))?;
    let record: Record = get(action_hash, GetOptions::local())?.unwrap();
    match record.entry() {
        RecordEntry::Present(entry) => hdk::prelude::hash_entry(entry.clone()),
        _ => unreachable!(),
    }
}

#[hdk_extern]
fn twenty_three_degrees_hash(_: ()) -> ExternResult<EntryHash> {
    hdk::prelude::hash_entry(&temperature())
}

#[hdk_extern]
fn hash_entry(entry: Entry) -> ExternResult<EntryHash> {
    hdk::prelude::hash_entry(entry)
}
