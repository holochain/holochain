use crate::integrity::*;
use hdk::prelude::*;
use EntryZomes::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityUpdateEntry(EntryTypes),
}

#[hdk_extern]
fn create_entry(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))
}

#[hdk_extern]
fn get_entry(_: ()) -> ExternResult<Option<Element>> {
    get(hash_entry(&post())?, GetOptions::latest())
}

#[hdk_extern]
fn update_entry(_: ()) -> ExternResult<ActionHash> {
    let action_hash = hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))?;
    hdk::prelude::update_entry(action_hash, &post())
}

#[hdk_extern]
/// Updates to a different entry, this will fail
fn invalid_update_entry(_: ()) -> ExternResult<ActionHash> {
    let action_hash = hdk::prelude::create_entry(&IntegrityUpdateEntry(post()))?;
    hdk::prelude::update_entry(action_hash, &msg())
}
