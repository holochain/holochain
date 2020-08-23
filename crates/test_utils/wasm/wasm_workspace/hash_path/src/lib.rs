use hdk3::prelude::*;
use test_wasm_common::TestBool;
use test_wasm_common::TestString;

entry_defs![Path::entry_def()];

#[hdk(extern)]
fn hash(path_string: TestString) -> ExternResult<EntryHash> {
    Path::from(path_string.0).hash()
}

#[hdk(extern)]
fn exists(path_string: TestString) -> ExternResult<TestBool> {
    Ok(Path::from(path_string.0).exists()?.into())
}

#[hdk(extern)]
fn ensure(path_string: TestString) -> ExternResult<()> {
    Path::from(path_string.0).ensure()
}

#[hdk(extern)]
fn remove_link(remove_link: RemoveLinkInput) -> ExternResult<HeaderHash> {
    Ok(remove_link!(remove_link.into_inner())?)
}

#[hdk(extern)]
fn children(path_string: TestString) -> ExternResult<Links> {
    Path::from(path_string.0).children()
}

#[hdk(extern)]
fn children_details(path_string: TestString) -> ExternResult<LinkDetails> {
    Path::from(path_string.0).children_details()
}
