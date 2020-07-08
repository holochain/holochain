use hdk3::hash_path::path::Path;
use hdk3::hash_path::{self, anchor::Anchor};
use hdk3::prelude::*;
use link::LinkTag;
use test_wasm_common::TestString;

holochain_externs!();

fn _entry_defs(_: ()) -> Result<EntryDefsCallbackResult, WasmError> {
    let mut defs = vec![Path::entry_def(), Anchor::entry_def()];
    Ok(EntryDefsCallbackResult::Defs(
        globals!()?.zome_name,
        defs.into(),
    ))
}
map_extern!(entry_defs, _entry_defs);

fn _anchor(anchor: Anchor) -> Result<HoloHashCore, WasmError> {
    debug!(&anchor.anchor_type)?;
    debug!(&anchor.anchor_text)?;
    anchor.touch()?;
    Ok(anchor.pwd()?)
}

map_extern!(anchor, _anchor);

fn _get_anchor(address: HoloHashCore) -> Result<Option<Anchor>, WasmError> {
    let entry = get_entry!(address)?;
    Ok(match entry {
        Some(Entry::App(serialized_bytes)) => Some(Anchor::try_from(serialized_bytes)?),
        _ => None,
    })
}
map_extern!(get_anchor, _get_anchor);

fn _list_anchor_type_address() -> Result<Vec<HoloHashCore>, WasmError> {
    let links = Path::from(hash_path::anchor::ROOT)
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}
map_extern!(list_anchor_type_address, _list_anchor_type_address);

fn _list_anchor_type_tags() -> Result<Vec<LinkTag>, WasmError> {
    let links = Path::from(hash_path::anchor::ROOT)
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.tag)
        .collect();
    Ok(links)
}
map_extern!(list_anchor_type_tags, _list_anchor_type_tags);

fn _list_anchor_addresses(anchor_type: TestString) -> Result<Vec<HoloHashCore>, WasmError> {
    let anchor = Anchor {
        anchor_type: anchor_type.0,
        anchor_text: None,
    };
    anchor.touch()?;
    let links = anchor
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.target)
        .collect();
    Ok(links)
}
map_extern!(list_anchor_addresses, _list_anchor_addresses);

fn _list_anchor_tags(anchor_type: TestString) -> Result<Vec<LinkTag>, WasmError> {
    let anchor = Anchor {
        anchor_type: anchor_type.0,
        anchor_text: None,
    };
    anchor.touch()?;
    let links = anchor
        .ls()?
        .into_inner()
        .into_iter()
        .map(|link| link.tag)
        .collect();
    Ok(links)
}
map_extern!(list_anchor_tags, _list_anchor_tags);
