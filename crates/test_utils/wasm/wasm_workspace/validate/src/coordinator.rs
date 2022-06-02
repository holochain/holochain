use crate::integrity::*;
use hdk::prelude::*;

impl TryFrom<&ThisWasmEntry> for CreateInput {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Ok(Self::new(
            EntryDefIndex::try_from(this_wasm_entry)?,
            EntryVisibility::Public,
            Entry::try_from(this_wasm_entry)?,
            ChainTopOrdering::default(),
        ))
    }
}

fn _commit_validate(to_commit: ThisWasmEntry) -> ExternResult<HeaderHash> {
    create((&to_commit).try_into()?)
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn always_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::AlwaysValidates)
}

#[hdk_extern]
fn never_validates(_: ()) -> ExternResult<HeaderHash> {
    _commit_validate(ThisWasmEntry::NeverValidates)
}
