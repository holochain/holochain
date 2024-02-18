use crate::integrity::*;
use hdk::prelude::*;

#[hdk_extern]
pub fn create_linked(content: u32) -> ExternResult<Record> {
    let thing = Thing { content };
    let thing_hash = create_entry(EntryTypes::Thing(thing))?;
    let record = get(thing_hash.clone(), GetOptions::content())?.ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("Could not find the newly created Thing"))
    ))?;

    let entry_hash = hash_entry(
        record
            .entry()
            .as_option()
            .ok_or_else(|| wasm_error!(WasmErrorInner::Guest(String::from("Missing entry hash"))))?
            .clone(),
    )?;

    tracing::info!("Creating link to {:?}", entry_hash);

    let my_agent_info = agent_info()?;
    create_link(
        my_agent_info.agent_latest_pubkey,
        entry_hash,
        LinkTypes::SomeLink,
        (),
    )?;

    Ok(record)
}

#[hdk_extern]
pub fn get_linked(_: ()) -> ExternResult<Vec<Link>> {
    let my_agent_info = agent_info()?;

    let links = get_links(GetLinksInputBuilder::try_new(my_agent_info.agent_latest_pubkey, LinkTypes::SomeLink)?.build())?;

    Ok(links)
}
