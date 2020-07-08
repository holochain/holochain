use hdk3::hash_path::path::Path;
use hdk3::prelude::*;
use test_wasm_common::TestBool;
use test_wasm_common::TestString;

holochain_externs!();

entry_defs!(vec![Path::entry_def()]);

map_extern!(pwd, _pwd);
map_extern!(exists, _exists);
map_extern!(touch, _touch);
map_extern!(ls, _ls);

fn _pwd(path_string: TestString) -> Result<HoloHashCore, WasmError> {
    Path::from(path_string.0).pwd()
}

fn _exists(path_string: TestString) -> Result<TestBool, WasmError> {
    Ok(Path::from(path_string.0).exists()?.into())
}

fn _touch(path_string: TestString) -> Result<(), WasmError> {
    Path::from(path_string.0).touch()
}

fn _ls(path_string: TestString) -> Result<holochain_zome_types::link::Links, WasmError> {
    Path::from(path_string.0).ls()
}

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
