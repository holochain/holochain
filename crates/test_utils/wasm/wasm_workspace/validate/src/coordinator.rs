use crate::integrity::*;
use hdk::prelude::*;

impl TryFrom<&ThisWasmEntry> for CreateInput {
    type Error = WasmError;
    fn try_from(this_wasm_entry: &ThisWasmEntry) -> Result<Self, Self::Error> {
        Self::new(
            EntryInput::App(AppEntry {
                entry_def_index: EntryDefIndex::try_from(this_wasm_entry)?,
                visibility: EntryVisibility::Public,
                entry: AppEntryBytes::try_from(this_wasm_entry)?,
            }),
            ChainTopOrdering::default(),
        )
    }
}

fn _commit_validate(to_commit: ThisWasmEntry) -> ExternResult<ActionHash> {
    create((&to_commit).try_into()?)
}

#[hdk_extern]
fn must_get_valid_record(action_hash: ActionHash) -> ExternResult<Record> {
    hdk::prelude::must_get_valid_record(action_hash)
}

#[hdk_extern]
fn always_validates(_: ()) -> ExternResult<ActionHash> {
    _commit_validate(ThisWasmEntry::AlwaysValidates)
}

#[hdk_extern]
fn never_validates(_: ()) -> ExternResult<ActionHash> {
    _commit_validate(ThisWasmEntry::NeverValidates)
}
