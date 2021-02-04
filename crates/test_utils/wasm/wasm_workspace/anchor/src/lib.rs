use hdk3::prelude::*;
use holochain_test_wasm_common::*;

entry_defs![Anchor::entry_def()];

#[hdk_extern]
fn anchor(input: AnchorInput) -> ExternResult<EntryHash> {
    hdk3::prelude::anchor(input.0, input.1)
}

#[hdk_extern]
fn get_anchor(address: EntryHash) -> ExternResult<Option<Anchor>> {
    hdk3::prelude::get_anchor(address)
}

#[hdk_extern]
fn list_anchor_type_addresses(_: ()) -> ExternResult<Vec<EntryHash>> {
    hdk3::prelude::list_anchor_type_addresses()
}

#[hdk_extern]
fn list_anchor_addresses(anchor_type: String) -> ExternResult<Vec<EntryHash>> {
    hdk3::prelude::list_anchor_addresses(
        anchor_type,
    )
}

#[hdk_extern]
fn list_anchor_tags(anchor_type: String) -> ExternResult<Vec<String>> {
    hdk3::prelude::list_anchor_tags(anchor_type)
}
