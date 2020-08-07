use hdk3::prelude::*;
use test_wasm_common::TestBool;
use test_wasm_common::TestString;

holochain_externs!();

entry_defs!(vec![Path::entry_def()]);

map_extern!(hash, _hash);
map_extern!(exists, _exists);
map_extern!(ensure, _ensure);
map_extern!(remove_link, _remove_link);
map_extern!(children, _children);
map_extern!(children_details, _children_details);

fn _hash(path_string: TestString) -> Result<EntryHash, WasmError> {
    Path::from(path_string.0).hash()
}

fn _exists(path_string: TestString) -> Result<TestBool, WasmError> {
    Ok(Path::from(path_string.0).exists()?.into())
}

fn _ensure(path_string: TestString) -> Result<(), WasmError> {
    Path::from(path_string.0).ensure()
}

fn _remove_link(remove_link: RemoveLinkInput) -> Result<HeaderHash, WasmError> {
    Ok(remove_link!(remove_link.into_inner())?)
}

fn _parent(path_string: TestString) -> Result<Option<Path>, WasmError> {
    Ok(Path::from(path_string.0).parent())
}

fn _children(path_string: TestString) -> Result<Links, WasmError> {
    Path::from(path_string.0).children()
}

fn _children_details(path_string: TestString) -> Result<LinkDetails, WasmError> {
    Path::from(path_string.0).children_details()
}
