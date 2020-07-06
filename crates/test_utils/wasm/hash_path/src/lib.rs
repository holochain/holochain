use hdk3::hash_path::path::Path;
use hdk3::prelude::*;
use test_wasm_common::TestBool;
use test_wasm_common::TestString;

holochain_externs!();

fn _entry_defs(_: ()) -> Result<EntryDefsCallbackResult, WasmError> {
    Ok(EntryDefsCallbackResult::Defs(
        globals!()?.zome_name,
        vec![Path::entry_def()].into(),
    ))
}
map_extern!(entry_defs, _entry_defs);

fn _pwd(path_string: TestString) -> Result<HoloHashCore, WasmError> {
    Path::from(path_string.0).pwd()
}
map_extern!(pwd, _pwd);

fn _exists(path_string: TestString) -> Result<TestBool, WasmError> {
    Ok(Path::from(path_string.0).exists()?.into())
}
map_extern!(exists, _exists);

fn _touch(path_string: TestString) -> Result<(), WasmError> {
    debug!(&path_string)?;
    Path::from(path_string.0).touch()
}
map_extern!(touch, _touch);

fn _ls(path_string: TestString) -> Result<holochain_zome_types::link::Links, WasmError> {
    Path::from(path_string.0).ls()
}
map_extern!(ls, _ls);

#[test]
#[cfg(test)]
fn hash_path_delimiter() {
    assert_eq!(hdk3::hash_path::path::DELIMITER, "/",);
}

#[test]
#[cfg(test)]
fn hash_path_name() {
    assert_eq!(hdk3::hash_path::path::NAME, "hdk.path".as_bytes(),);
}
