use hdk3::prelude::*;

#[hdk_entry(id = "generic_entry", visibility = "public")]
#[derive(Clone)]
pub struct GenericEntry(String);

entry_defs![GenericEntry::entry_def()];

#[hdk_extern]
fn create_entry(entry: GenericEntry) -> ExternResult<EntryHash> {
    commit_entry!(entry.clone())?;

    let hash = entry_hash!(entry.clone())?;

    Ok(hash)
}

#[hdk_extern]
fn get_entry(address: EntryHash) -> ExternResult<SerializedBytes> {
    let maybe_element = get!(address)?;

    try_from_element(maybe_element)
}

pub fn try_from_element(maybe_element: Option<Element>) -> ExternResult<SerializedBytes> {
    match maybe_element {
        None => Err(HdkError::Wasm(WasmError::Zome("Could not convert".into()))),
        Some(element) => match element.entry() {
            element::ElementEntry::Present(entry) => try_from_entry(entry.clone()),
            _ => Err(HdkError::Wasm(WasmError::Zome("Could not convert".into()))),
        },
    }
}

fn try_from_entry(entry: Entry) -> ExternResult<SerializedBytes> {
    match entry {
        Entry::App(content) => Ok(content.into_sb()),
        _ => Err(HdkError::Wasm(WasmError::Zome("Could not convert".into()))),
    }
}
