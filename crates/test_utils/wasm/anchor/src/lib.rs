use hdk3::prelude::*;
use link::LinkTag;
use test_wasm_common::TestString;

holochain_externs!();

entry_defs!(vec![Anchor::entry_def()]);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct AnchorInput(String, String);
fn _anchor(input: AnchorInput) -> Result<HoloHashCore, WasmError> {
    hdk3::prelude::anchor(input.0, input.1)
}
map_extern!(anchor, _anchor);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
struct MaybeAnchor(Option<Anchor>);
fn _get_anchor(address: HoloHashCore) -> Result<MaybeAnchor, WasmError> {
    Ok(MaybeAnchor(hdk3::prelude::get_anchor(address)?))
}
map_extern!(get_anchor, _get_anchor);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
struct Hashes(Vec<HoloHashCore>);
fn _list_anchor_type_addresses(_: ()) -> Result<Hashes, WasmError> {
    Ok(Hashes(hdk3::prelude::list_anchor_type_addresses()?))
}
map_extern!(list_anchor_type_addresses, _list_anchor_type_addresses);

fn _list_anchor_addresses(anchor_type: TestString) -> Result<Hashes, WasmError> {
    Ok(Hashes(hdk3::prelude::list_anchor_addresses(anchor_type.0)?))
}
map_extern!(list_anchor_addresses, _list_anchor_addresses);

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
#[repr(transparent)]
#[serde(transparent)]
struct LinkTags(Vec<LinkTag>);
fn _list_anchor_tags(anchor_type: TestString) -> Result<LinkTags, WasmError> {
    Ok(LinkTags(hdk3::prelude::list_anchor_tags(anchor_type.0)?))
}
map_extern!(list_anchor_tags, _list_anchor_tags);
