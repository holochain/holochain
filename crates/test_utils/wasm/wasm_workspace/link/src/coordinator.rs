use crate::integrity::LinkTypes;
use hdk::prelude::*;

#[hdk_dependent_link_types]
enum LinkZomes {
    IntegrityLink(LinkTypes),
    IntegrityLink2(LinkTypes),
}

fn path(s: &str) -> ExternResult<AnyLinkableHash> {
    let path = Path::from(s).typed(LinkTypes::SomeLinks)?;
    path.ensure()?;
    Ok(path.path_entry_hash()?.into())
}

fn base() -> ExternResult<AnyLinkableHash> {
    path("a")
}

fn baseless() -> ExternResult<AnyLinkableHash> {
    Ok(EntryHash::from_raw_36([1_u8; 36].to_vec()).into())
}

fn target() -> ExternResult<AnyLinkableHash> {
    path("b")
}

fn external() -> ExternResult<AnyLinkableHash> {
    Ok(ExternalHash::from_raw_36([0_u8; 36].to_vec()).into())
}

fn targetless() -> ExternResult<AnyLinkableHash> {
    Ok(EntryHash::from_raw_36([2_u8; 36].to_vec()).into())
}

#[hdk_extern]
fn create_link(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(base()?, target()?, LinkTypes::SomeLinks, ())
}

#[hdk_extern]
fn create_nested_link(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(
        base()?,
        target()?,
        LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
        (),
    )
}

#[hdk_extern]
fn create_baseless_link(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(baseless()?, targetless()?, LinkTypes::SomeLinks, ())
}

#[hdk_extern]
fn create_external_base_link(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(external()?, base()?, LinkTypes::SomeLinks, ())
}

#[hdk_extern]
fn create_back_link(_: ()) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(target()?, base()?, LinkTypes::SomeLinks, ())
}

#[hdk_extern]
fn delete_link(input: ActionHash) -> ExternResult<ActionHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(base()?, LinkTypes::SomeLinks, None)?;
    // Include all links from within this zome.
    hdk::prelude::get_links(base()?, .., None)?;
    // Include types in this vec.
    hdk::prelude::get_links(
        base()?,
        vec![LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks],
        None,
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        base()?,
        [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks],
        None,
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        base()?,
        &[LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks],
        None,
    )?;
    let t = [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks];
    // Include types in this slice.
    hdk::prelude::get_links(base()?, &t[..], None)
}

#[hdk_extern]
fn get_links_nested(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(
        base()?,
        LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
        None,
    )?;
    // Include all links from within this zome.
    hdk::prelude::get_links(base()?, .., None)?;
    // Include types in this vec.
    hdk::prelude::get_links(
        base()?,
        vec![
            LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
            LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
        ],
        None,
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        base()?,
        [
            LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
            LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
        ],
        None,
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        base()?,
        &[
            LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
            LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
        ],
        None,
    )?;
    let t = [
        LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
        LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
    ];
    // Include types in this slice.
    hdk::prelude::get_links(base()?, &t[..], None)
    // Include all link types defined in any zome.
}

#[hdk_extern]
fn get_baseless_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(baseless()?, LinkTypes::SomeLinks, None)
}

#[hdk_extern]
fn get_external_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(external()?, LinkTypes::SomeLinks, None)
}

#[hdk_extern]
fn get_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, LinkTypes::SomeLinks, None)
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(target()?, LinkTypes::SomeLinks, None)
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(target()?, LinkTypes::SomeLinks, None)
}

#[hdk_extern]
fn get_links_bidi(_: ()) -> ExternResult<Vec<Vec<Link>>> {
    HDK.with(|h| {
        h.borrow().get_links(vec![
            GetLinksInput::new(base()?, LinkTypes::SomeLinks.try_into()?, None),
            GetLinksInput::new(target()?, LinkTypes::SomeLinks.try_into()?, None),
        ])
    })
}

#[hdk_extern]
fn get_link_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| {
        h.borrow().get_link_details(vec![
            GetLinksInput::new(base()?, LinkTypes::SomeLinks.try_into()?, None),
            GetLinksInput::new(target()?, LinkTypes::SomeLinks.try_into()?, None),
        ])
    })
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(base()?, LinkTypes::SomeLinks, None)? {
        hdk::prelude::delete_link(link.create_link_hash)?;
    }
    Ok(())
}

/// Same as path.ensure() but doesn't check for
/// exists. This can happen when ensuring paths
/// in partitions so this test just shows that it's safe to do so.
#[hdk_extern]
fn commit_existing_path(_: ()) -> ExternResult<()> {
    let path = Path::from("a.c").typed(LinkTypes::SomeLinks)?;
    if let Some(parent) = path.parent() {
        parent.ensure()?;
        hdk::prelude::create_link(
            parent.path_entry_hash()?,
            path.path_entry_hash()?,
            LinkTypes::SomeLinks,
            LinkTag::new(
                match path.leaf() {
                    None => <Vec<u8>>::new(),
                    Some(component) => UnsafeBytes::from(
                        SerializedBytes::try_from(component).map_err(|e| wasm_error!(e))?,
                    )
                    .into(),
                }
                .to_vec(),
            ),
        )?;
    }
    Ok(())
}

#[hdk_extern]
fn get_long_path(_: ()) -> ExternResult<Vec<Link>> {
    Path::from("a").typed(LinkTypes::SomeLinks)?.children()
}
