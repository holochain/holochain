use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn create_thing(content: u32) -> ExternResult<Record> {
    let thing = Thing { content };
    let thing_hash = create_entry(EntryTypes::Thing(thing))?;
    let record = get(thing_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("Could not find the newly created Thing"))
    ))?;
    Ok(record)
}

#[hdk_extern]
pub fn get_thing(thing_hash: ActionHash) -> ExternResult<Option<Record>> {
    get(thing_hash, GetOptions::default())
}
