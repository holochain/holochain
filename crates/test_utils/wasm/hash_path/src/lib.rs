use hdk3::hash_path::path::Path;
use hdk3::prelude::*;
use test_wasm_common::TestBool;
use test_wasm_common::TestString;

holochain_externs!();
host_externs!(__get);

entry_defs!(vec![Path::entry_def()]);

map_extern!(hash, _hash);
map_extern!(exists, _exists);
map_extern!(ensure, _ensure);
map_extern!(children, _children);

fn _hash(path_string: TestString) -> Result<EntryHash, WasmError> {
    Path::from(path_string.0).hash()
}

fn _exists(path_string: TestString) -> Result<TestBool, WasmError> {
    Ok(Path::from(path_string.0).exists()?.into())
}

fn _ensure(path_string: TestString) -> Result<(), WasmError> {
    Path::from(path_string.0).ensure()
}

fn _parent(path_string: TestString) -> Result<Option<Path>, WasmError> {
    Ok(Path::from(path_string.0).parent())
}

fn _children(path_string: TestString) -> Result<holochain_zome_types::link::Links, WasmError> {
    Path::from(path_string.0).children()
}
