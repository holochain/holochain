use hdk3::prelude::*;
use holochain_test_wasm_common::*;

entry_defs![Anchor::entry_def()];

#[hdk_extern]
fn anchor(input: AnchorInput) -> ExternResult<EntryHash> {
    hdk3::prelude::anchor(input.0, input.1)
}

#[hdk_extern]
fn get_anchor(address: EntryHash) -> ExternResult<MaybeAnchor> {
    Ok(MaybeAnchor(hdk3::prelude::get_anchor(address)?))
}

#[hdk_extern]
fn list_anchor_type_addresses(_: ()) -> ExternResult<EntryHashes> {
    Ok(EntryHashes(hdk3::prelude::list_anchor_type_addresses()?))
}

#[hdk_extern]
fn list_anchor_addresses(anchor_type: TestString) -> ExternResult<EntryHashes> {
    Ok(EntryHashes(hdk3::prelude::list_anchor_addresses(
        anchor_type.0,
    )?))
}

#[hdk_extern]
fn list_anchor_tags(anchor_type: TestString) -> ExternResult<AnchorTags> {
    Ok(AnchorTags(hdk3::prelude::list_anchor_tags(anchor_type.0)?))
}
