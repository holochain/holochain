use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
fn create(_: ()) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::MyType(MyType {
        value: "test".to_string(),
        amount: 4,
    }))
}

#[hdk_extern]
fn init() -> ExternResult<()> {
    // TODO Need to get previous DNA hash from somewhere
    Ok(())
}

#[hdk_extern]
fn open_chain_from_prev(prev_dna_hash: DnaHash) -> ExternResult<ActionHash> {
    open_chain(prev_dna_hash)
}
