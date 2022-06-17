use hdk::prelude::*;

mod countree;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityCrud(crate::integrity::EntryTypes),
}

#[hdk_extern]
fn new(_: ()) -> ExternResult<ActionHash> {
    countree::CounTree::new()
}

#[hdk_extern]
fn action_details(action_hashes: Vec<ActionHash>) -> ExternResult<Vec<Option<Details>>> {
    countree::CounTree::action_details(action_hashes)
}

#[hdk_extern]
fn entry_details(entry_hashes: Vec<EntryHash>) -> ExternResult<Vec<Option<Details>>> {
    countree::CounTree::entry_details(entry_hashes)
}

#[hdk_extern]
fn entry_hash(countree: countree::CounTree) -> ExternResult<EntryHash> {
    hash_entry(&countree)
}

#[hdk_extern]
fn inc(action_hash: ActionHash) -> ExternResult<ActionHash> {
    countree::CounTree::incsert(action_hash)
}

#[hdk_extern]
fn dec(action_hash: ActionHash) -> ExternResult<ActionHash> {
    countree::CounTree::dec(action_hash)
}
