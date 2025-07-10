use crate::integrity::*;
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
fn delete_link(create_link_hash: ActionHash) -> ExternResult<ActionHash> {
    hdk::prelude::delete_link(create_link_hash, GetOptions::default())
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(
        LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )?;
    // Include all links from within this zome.
    hdk::prelude::get_links(
        LinkQuery::new(base()?, (..).try_into_filter()?),
        GetStrategy::default(),
    )?;
    // Include types in this vec.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            vec![LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks].try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks].try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            (&[LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks]).try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    let t = [LinkTypes::SomeLinks, LinkTypes::SomeOtherLinks];
    // Include types in this slice.
    hdk::prelude::get_links(
        LinkQuery::new(base()?, (&t[..]).try_into_filter()?),
        GetStrategy::default(),
    )
}

#[hdk_extern]
fn get_links_nested(_: ()) -> ExternResult<Vec<Link>> {
    // Include just `SomeLinks`
    hdk::prelude::get_links(
        LinkQuery::try_new(base()?, LinkZomes::IntegrityLink(LinkTypes::SomeLinks))?,
        GetStrategy::default(),
    )?;
    // Include all links from within this zome.
    hdk::prelude::get_links(
        LinkQuery::new(base()?, (..).try_into_filter()?),
        GetStrategy::default(),
    )?;
    // Include types in this vec.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            vec![
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ]
            .try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    // Include types in this array.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            [
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ]
            .try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    // Include types in this ref to array.
    hdk::prelude::get_links(
        LinkQuery::new(
            base()?,
            (&[
                LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
                LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
            ])
                .try_into_filter()?,
        ),
        GetStrategy::default(),
    )?;
    let t = [
        LinkZomes::IntegrityLink(LinkTypes::SomeLinks),
        LinkZomes::IntegrityLink(LinkTypes::SomeOtherLinks),
    ];
    // Include types in this slice.
    hdk::prelude::get_links(
        LinkQuery::new(base()?, (&t[..]).try_into_filter()?),
        GetStrategy::default(),
    )
    // Include all link types defined in any zome.
}

#[hdk_extern]
fn get_baseless_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(
        LinkQuery::try_new(baseless()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )
}

#[hdk_extern]
fn get_external_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(
        LinkQuery::try_new(external()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )
}

#[hdk_extern]
fn get_links_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_links_details(
        LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(
        LinkQuery::try_new(target()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_links_details(
        LinkQuery::try_new(target()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )
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
fn get_links_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| {
        h.borrow().get_links_details(vec![
            GetLinksInputBuilder::try_new(base()?, LinkTypes::SomeLinks)?.build(),
            GetLinksInputBuilder::try_new(target()?, LinkTypes::SomeLinks)?.build(),
        ])
    })
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(
        LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?,
        GetStrategy::default(),
    )? {
        hdk::prelude::delete_link(link.create_link_hash, GetOptions::default())?;
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
fn get_links_with_query(link_query: LinkQuery) -> ExternResult<Vec<Link>> {
    Ok(hdk::prelude::get_links(link_query, GetStrategy::default())?)
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
    let get_links_input = LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?;
    hdk::prelude::get_links(get_links_input, GetStrategy::Local)
}

#[hdk_extern]
fn get_links_details_local_only(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_links_details(
        LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?,
        GetStrategy::Local,
    )
}

#[hdk_extern]
fn get_links_from_network(_: ()) -> ExternResult<Vec<Link>> {
    let get_links_input = LinkQuery::try_new(base()?, LinkTypes::SomeLinks)?;
    hdk::prelude::get_links(get_links_input, GetStrategy::Network)
}

#[hdk_extern]
fn test_entry_create() -> ExternResult<ActionHash> {
    create_entry(&EntryTypes::Test(Test))
}

#[hdk_extern]
fn link_validation_calls_must_get_valid_record(
    input: (ActionHash, AgentPubKey),
) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(
        input.0,
        input.1,
        LinkTypes::LinkValidationCallsMustGetValidRecord,
        (),
    )
}

#[hdk_extern]
fn link_validation_calls_must_get_action_then_entry(
    input: (ActionHash, AgentPubKey),
) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(
        input.0,
        input.1,
        LinkTypes::LinkValidationCallsMustGetActionThenEntry,
        (),
    )
}

#[hdk_extern]
fn link_validation_calls_must_get_agent_activity(
    input: (ActionHash, AgentPubKey),
) -> ExternResult<ActionHash> {
    hdk::prelude::create_link(
        input.0,
        input.1,
        LinkTypes::LinkValidationCallsMustGetAgentActivity,
        (),
    )
}
