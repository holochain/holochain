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
fn create_tagged_link(tag: String) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(
        base()?,
        target()?,
        LinkTypes::SomeLinks,
        tag.as_bytes().to_vec(),
    )
}

#[hdk_extern]
fn delete_link(input: ActionHash) -> ExternResult<ActionHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?.build())?;
    // Include all links from within this zome.
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(base()?, ..)?.build())?;
    // Include types in this vec.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(
            base()?,
            vec![LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks],
        )?
        .build(),
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(base()?, [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks])?
            .build(),
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(base()?, &[LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks])?
            .build(),
    )?;
    let t = [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks];
    // Include types in this slice.
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(base()?, &t[..])?.build())
}

#[hdk_extern]
fn get_links_nested(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(base()?, LinkZomes::IntegrityLink(LinkTypes::SomeLinks))?
            .build(),
    )?;
    // Include all links from within this zome.
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(base()?, ..)?.build())?;
    // Include types in this vec.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(
            base()?,
            vec![
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ],
        )?
        .build(),
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(
            base()?,
            [
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ],
        )?
        .build(),
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(
            base()?,
            &[
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ],
        )?
        .build(),
    )?;
    let t = [
        LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
        LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
    ];
    // Include types in this slice.
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(base()?, &t[..])?.build())
    // Include all link types defined in any zome.
}

#[hdk_extern]
fn get_baseless_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(baseless()?, LinkTypes::SomeLinks)?.build(),
    )
}

#[hdk_extern]
fn get_external_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(external()?, LinkTypes::SomeLinks)?.build(),
    )
}

#[hdk_extern]
fn get_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, LinkTypes::SomeLinks, None, GetOptions::default())
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(GetLinksInputBuilder::try_new(target()?, LinkTypes::SomeLinks)?.build())
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(target()?, LinkTypes::SomeLinks, None, GetOptions::default())
}

#[hdk_extern]
fn get_links_bidi(_: ()) -> ExternResult<Vec<Vec<Link>>> {
    HDK.with(|h| {
        h.borrow().get_links(vec![
            GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?.build(),
            GetLinksInputBuilder::try_new(target()?, LinkTypes::SomeLinks)?.build(),
        ])
    })
}

#[hdk_extern]
fn get_link_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| {
        h.borrow().get_link_details(vec![
            GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?.build(),
            GetLinksInputBuilder::try_new(target()?, LinkTypes::SomeLinks)?.build(),
        ])
    })
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(
        GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?.build(),
    )? {
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

#[hdk_extern]
fn get_base_hash(_: ()) -> ExternResult<AnyLinkableHash> {
    base()
}

#[hdk_extern]
fn get_count(link_query: LinkQuery) -> ExternResult<usize> {
    hdk::prelude::count_links(link_query)
}

#[hdk_extern]
fn get_links_with_query(input: GetLinksInput) -> ExternResult<Vec<Link>> {
    Ok(hdk::prelude::get_links(input)?)
}

#[hdk_extern]
fn get_time(_: ()) -> ExternResult<Timestamp> {
    sys_time()
}

#[hdk_extern]
fn get_path_hash(s: String) -> ExternResult<AnyLinkableHash> {
    path(s.as_str())
}

#[hdk_extern]
fn get_links_local_only(_: ()) -> ExternResult<Vec<Link>> {
    let get_links_input = GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?
        .get_options(GetStrategy::Local)
        .build();
    hdk::prelude::get_links(get_links_input)
}

#[hdk_extern]
fn get_link_details_local_only(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, LinkTypes::SomeLinks, None, GetOptions::local())
}

#[hdk_extern]
fn get_links_from_network(_: ()) -> ExternResult<Vec<Link>> {
    let get_links_input = GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?
        .get_options(GetStrategy::Network)
        .build();
    hdk::prelude::get_links(get_links_input)
}
