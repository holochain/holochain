use hdk::prelude::*;
use hdk::hash_path::path::DHT_PREFIX;

entry_defs![
    Path::entry_def(),
    PathEntry::entry_def()
];

fn path(s: &str) -> ExternResult<EntryHash> {
    let path = Path::from(s);
    path.ensure()?;
    path.path_entry_hash()
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
fn create_back_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(target()?, base()?, ())
}

#[hdk_extern]
fn delete_link(input: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(base()?, None)
}

#[hdk_extern]
fn get_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, None)
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(target()?, None)
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(target()?, None)
}

#[hdk_extern]
fn get_links_bidi(_: ()) -> ExternResult<Vec<Vec<Link>>> {
    HDK.with(|h| h.borrow().get_links(vec![
        GetLinksInput::new(base()?, None),
        GetLinksInput::new(target()?, None),
    ]))
}

#[hdk_extern]
fn get_link_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| h.borrow().get_link_details(vec![
        GetLinksInput::new(base()?, None),
        GetLinksInput::new(target()?, None),
    ]))
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(base()?, None)? {
        hdk::prelude::delete_link(link.create_link_hash)?;
    }
    Ok(())
}

/// Same as path.ensure() but doesn't check for
/// exists. This can happen when ensuring paths
/// in partitions so this test just shows that it's safe to do so.
#[hdk_extern]
fn commit_existing_path(_: ()) -> ExternResult<()> {
    let path = Path::from("a.c");
    create_entry(&path.path_entry()?)?;
    if let Some(parent) = path.parent() {
        parent.ensure()?;
        hdk::prelude::create_link(parent.path_entry_hash()?, path.path_entry_hash()?, LinkTag::new(
            [DHT_PREFIX].iter()
                .chain(match path.leaf() {
                    None => <Vec<u8>>::new(),
                    Some(component) => UnsafeBytes::from(SerializedBytes::try_from(component)?).into(),
                }.iter())
                .cloned()
                .collect::<Vec<u8>>(),
        ))?;
    }
    Ok(())
}

#[hdk_extern]
fn get_long_path(_: ()) -> ExternResult<Vec<Link>> {
    Path::from("a").children()
}
