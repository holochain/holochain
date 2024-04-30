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
fn init() -> ExternResult<ActionHash> {
    // open_chain(dna_hash)
    todo!("Need to get previous DNA hash from somewhere!")
}
