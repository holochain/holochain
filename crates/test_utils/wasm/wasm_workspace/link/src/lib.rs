use hdk3::prelude::*;

entry_defs![Path::entry_def()];

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s);
    path.ensure()?;
    Ok(path.hash()?)
}

fn base() -> ExternResult<EntryHash> {
    path("a")
}

fn target() -> ExternResult<EntryHash> {
    path("b")
}

#[hdk_extern]
fn create_link(_: ()) -> ExternResult<HeaderHash> {
    Ok(create_link!(base()?, target()?)?)
}

#[hdk_extern]
fn delete_link(input: DeleteLinkInput) -> ExternResult<HeaderHash> {
    Ok(delete_link!(input.into_inner())?)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Links> {
    Ok(get_links!(base()?)?)
}
