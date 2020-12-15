use hdk3::prelude::*;
use holochain_test_wasm_common::TestBool;
use holochain_test_wasm_common::TestString;

entry_defs![Path::entry_def()];

#[hdk_extern]
fn hash(path_string: TestString) -> ExternResult<EntryHash> {
    Path::from(path_string.0).hash()
}

#[hdk_extern]
fn exists(path_string: TestString) -> ExternResult<TestBool> {
    Ok(Path::from(path_string.0).exists()?.into())
}

#[hdk_extern]
fn ensure(path_string: TestString) -> ExternResult<()> {
    Path::from(path_string.0).ensure()
}

#[hdk_extern]
fn delete_link(delete_link: DeleteLinkInput) -> ExternResult<HeaderHash> {
    Ok(hdk3::prelude::delete_link(delete_link.into_inner())?)
}

#[hdk_extern]
fn children(path_string: TestString) -> ExternResult<Links> {
    Path::from(path_string.0).children()
}

#[hdk_extern]
fn children_details(path_string: TestString) -> ExternResult<LinkDetails> {
    Path::from(path_string.0).children_details()
}
