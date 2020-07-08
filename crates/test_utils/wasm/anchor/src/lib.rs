use hdk3::prelude::*;
// use link::LinkTag;
// use test_wasm_common::TestString;

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

// fn _list_anchor_type_address() -> Result<Vec<HoloHashCore>, WasmError> {
//     let links = Path::from(hash_path::anchor::ROOT)
//         .ls()?
//         .into_inner()
//         .into_iter()
//         .map(|link| link.target)
//         .collect();
//     Ok(links)
// }
// map_extern!(list_anchor_type_address, _list_anchor_type_address);
//
// fn _list_anchor_type_tags() -> Result<Vec<LinkTag>, WasmError> {
//     let links = Path::from(hash_path::anchor::ROOT)
//         .ls()?
//         .into_inner()
//         .into_iter()
//         .map(|link| link.tag)
//         .collect();
//     Ok(links)
// }
// map_extern!(list_anchor_type_tags, _list_anchor_type_tags);
//
// fn _list_anchor_addresses(anchor_type: TestString) -> Result<Vec<HoloHashCore>, WasmError> {
//     let anchor = Anchor {
//         anchor_type: anchor_type.0,
//         anchor_text: None,
//     };
//     anchor.touch()?;
//     let links = anchor
//         .ls()?
//         .into_inner()
//         .into_iter()
//         .map(|link| link.target)
//         .collect();
//     Ok(links)
// }
// map_extern!(list_anchor_addresses, _list_anchor_addresses);
//
// fn _list_anchor_tags(anchor_type: TestString) -> Result<Vec<LinkTag>, WasmError> {
//     let anchor = Anchor {
//         anchor_type: anchor_type.0,
//         anchor_text: None,
//     };
//     anchor.touch()?;
//     let links = anchor
//         .ls()?
//         .into_inner()
//         .into_iter()
//         .map(|link| link.tag)
//         .collect();
//     Ok(links)
// }
// map_extern!(list_anchor_tags, _list_anchor_tags);
