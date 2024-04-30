use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
fn create(_: ()) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::MyType(MyType {
        value: "test".to_string(),
    }))
}

#[hdk_extern]
fn close_chain_for_new(dna_hash: DnaHash) -> ExternResult<ActionHash> {
    close_chain(dna_hash)
}
