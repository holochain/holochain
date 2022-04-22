use crate::integrity::LinkTypes;
use hdk::prelude::*;

#[derive(ToZomeName)]
enum Zomes {
    IntegrityLink,
}

#[hdk_link_zomes]
enum LinkZomes {
    IntegrityLink(LinkTypes),
}

fn path(s: &str) -> ExternResult<AnyLinkableHash> {
    let path = Path::from(s).locate(Zomes::IntegrityLink);
    path.ensure()?;
    Ok(path.path_entry_hash()?.into())
}

fn base() -> ExternResult<AnyLinkableHash> {
    path("a")
}

fn baseless() -> ExternResult<AnyLinkableHash> {
    Ok(EntryHash::from_raw_32([1_u8; 32].to_vec()).into())
}

fn target() -> ExternResult<AnyLinkableHash> {
    path("b")
}

fn external() -> ExternResult<AnyLinkableHash> {
    Ok(ExternalHash::from_raw_32([0_u8; 32].to_vec()).into())
}

fn targetless() -> ExternResult<AnyLinkableHash> {
    Ok(EntryHash::from_raw_32([2_u8; 32].to_vec()).into())
}

#[hdk_extern]
fn create_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(
        base()?,
        target()?,
        LinkZomes::IntegrityLink(LinkTypes::Any),
        (),
    )
}

#[hdk_extern]
fn create_baseless_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(
        baseless()?,
        targetless()?,
        LinkZomes::IntegrityLink(LinkTypes::Any),
        (),
    )
}

#[hdk_extern]
fn create_external_base_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(
        external()?,
        base()?,
        LinkZomes::IntegrityLink(LinkTypes::Any),
        (),
    )
}

#[hdk_extern]
fn create_back_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(
        target()?,
        base()?,
        LinkZomes::IntegrityLink(LinkTypes::Any),
        (),
    )
}

#[hdk_extern]
fn delete_link(input: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(base()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_baseless_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(baseless()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_external_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(external()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(target()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(target()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)
}

#[hdk_extern]
fn get_links_bidi(_: ()) -> ExternResult<Vec<Vec<Link>>> {
    HDK.with(|h| {
        h.borrow().get_links(vec![
            GetLinksInput::new(
                base()?,
                LinkZomes::IntegrityLink(LinkTypes::Any).into(),
                None,
            ),
            GetLinksInput::new(
                target()?,
                LinkZomes::IntegrityLink(LinkTypes::Any).into(),
                None,
            ),
        ])
    })
}

#[hdk_extern]
fn get_link_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| {
        h.borrow().get_link_details(vec![
            GetLinksInput::new(
                base()?,
                LinkZomes::IntegrityLink(LinkTypes::Any).into(),
                None,
            ),
            GetLinksInput::new(
                target()?,
                LinkZomes::IntegrityLink(LinkTypes::Any).into(),
                None,
            ),
        ])
    })
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(base()?, LinkZomes::IntegrityLink(LinkTypes::Any), None)? {
        hdk::prelude::delete_link(link.create_link_hash)?;
    }
    Ok(())
}

/// Same as path.ensure() but doesn't check for
/// exists. This can happen when ensuring paths
/// in partitions so this test just shows that it's safe to do so.
#[hdk_extern]
fn commit_existing_path(_: ()) -> ExternResult<()> {
    let path = Path::from("a.c").locate(Zomes::IntegrityLink);
    if let Some(parent) = path.parent() {
        parent.ensure()?;
        hdk::prelude::create_link(
            parent.path_entry_hash()?.into(),
            path.path_entry_hash()?.into(),
            LinkZomes::IntegrityLink(LinkTypes::Any),
            LinkTag::new(
                match path.leaf() {
                    None => <Vec<u8>>::new(),
                    Some(component) => {
                        UnsafeBytes::from(SerializedBytes::try_from(component)?).into()
                    }
                }
                .to_vec(),
            ),
        )?;
    }
    Ok(())
}

#[hdk_extern]
fn get_long_path(_: ()) -> ExternResult<Vec<Link>> {
    Path::from("a").locate(Zomes::IntegrityLink).children()
}
