use hdk::prelude::*;

#[hdk_extern]
fn must_get_valid_commit(action_hash: ActionHash) -> ExternResult<Commit> {
    hdk::prelude::must_get_valid_commit(action_hash)
}

#[hdk_extern]
fn must_get_action(action_hash: ActionHash) -> ExternResult<SignedActionHashed> {
    hdk::prelude::must_get_action(action_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}
