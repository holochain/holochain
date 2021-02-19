use hdk::prelude::*;

entry_defs![Path::entry_def()];

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s);
    path.ensure()?;
    path.hash()
}

fn base() -> ExternResult<EntryHash> {
    path("a")
}

fn target() -> ExternResult<EntryHash> {
    path("b")
}

#[hdk_extern]
fn create_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(base()?, target()?, ())
}

#[hdk_extern]
fn delete_link(input: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Links> {
    hdk::prelude::get_links(base()?, None)
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(base()?, None)?.into_inner() {
        hdk::prelude::delete_link(link.create_link_hash)?;
    }
    Ok(())
}

/// Same as path.ensure() but doesn't check for
/// exists. This can happen when ensuring paths
/// in partitions.
#[hdk_extern]
fn commit_existing_path(_: ()) -> ExternResult<()> {
    let path = Path::from("a.c");
    create_entry(&path)?;
    if let Some(parent) = path.parent() {
        parent.ensure()?;
        hdk::prelude::create_link(parent.hash()?, path.hash()?, LinkTag::try_from(&path)?)?;
    }
    Ok(())
}

#[hdk_extern]
fn get_long_path(_: ()) -> ExternResult<Links> {
    Path::from("a").children()
}
