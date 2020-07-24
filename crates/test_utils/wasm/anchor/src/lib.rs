use hdk3::prelude::*;
use test_wasm_common::*;

holochain_externs!();

entry_defs!(vec![Anchor::entry_def()]);

map_extern!(anchor, _anchor);
map_extern!(get_anchor, _get_anchor);
map_extern!(list_anchor_type_addresses, _list_anchor_type_addresses);
map_extern!(list_anchor_addresses, _list_anchor_addresses);
map_extern!(list_anchor_tags, _list_anchor_tags);

fn _anchor(input: AnchorInput) -> Result<EntryHash, WasmError> {
    hdk3::prelude::anchor(input.0, input.1)
}

fn _get_anchor(address: EntryHash) -> Result<MaybeAnchor, WasmError> {
    Ok(MaybeAnchor(hdk3::prelude::get_anchor(address)?))
}

fn _list_anchor_type_addresses(_: ()) -> Result<EntryHashes, WasmError> {
    Ok(EntryHashes(hdk3::prelude::list_anchor_type_addresses()?))
}

fn _list_anchor_addresses(anchor_type: TestString) -> Result<EntryHashes, WasmError> {
    Ok(EntryHashes(hdk3::prelude::list_anchor_addresses(
        anchor_type.0,
    )?))
}

fn _list_anchor_tags(anchor_type: TestString) -> Result<AnchorTags, WasmError> {
    Ok(AnchorTags(hdk3::prelude::list_anchor_tags(anchor_type.0)?))
}
