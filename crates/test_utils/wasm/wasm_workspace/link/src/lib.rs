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
    Ok(hdk3::prelude::create_link(base()?, target()?, ())?)
}

#[hdk_extern]
fn delete_link(input: DeleteLinkInput) -> ExternResult<HeaderHash> {
    Ok(delete_link!(input.into_inner())?)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Links> {
    Ok(get_links!(base()?)?)
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in get_links!(base()?)?.into_inner() {
        delete_link!(link.create_link_hash)?;
    }
    Ok(())
}
