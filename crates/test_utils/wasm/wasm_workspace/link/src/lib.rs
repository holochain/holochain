use hdk3::prelude::*;

entry_defs![Path::entry_def()];

fn path(s: &str) -> Result<EntryHash, WasmError> {
    let path = Path::from(s);
    path.ensure()?;
    Ok(path.hash()?)
}

fn base() -> Result<EntryHash, WasmError> {
    path("a")
}

fn target() -> Result<EntryHash, WasmError> {
    path("b")
}

#[hdk_extern]
fn link_entries(_: ()) -> ExternResult<HeaderHash> {
    Ok(link_entries!(base()?, target()?)?)
}

#[hdk_extern]
fn remove_link(input: RemoveLinkInput) -> ExternResult<HeaderHash> {
    Ok(remove_link!(input.into_inner())?)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Links> {
    Ok(get_links!(base()?)?)
}
