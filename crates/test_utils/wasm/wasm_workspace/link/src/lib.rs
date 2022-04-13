use hdk::prelude::*;

enum LinkTypes {
    Foo,
}

impl From<LinkTypes> for u8 {
    fn from(link_types: LinkTypes) -> Self {
        link_types as u8
    }
}

entry_defs![Path::entry_def(), PathEntry::entry_def()];

fn path(s: &str) -> ExternResult<AnyLinkableHash> {
    let path = Path::from(s);
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
    hdk::prelude::create_link(base()?, target()?, HdkLinkType::Any, ())
}

#[hdk_extern]
fn create_typed_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(base()?, targetless()?, LinkTypes::Foo as u8, ())
}

#[hdk_extern]
fn create_baseless_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(baseless()?, targetless()?, HdkLinkType::Any, ())
}

#[hdk_extern]
fn create_external_base_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(external()?, base()?, HdkLinkType::Any, ())
}

#[hdk_extern]
fn create_back_link(_: ()) -> ExternResult<HeaderHash> {
    hdk::prelude::create_link(target()?, base()?, HdkLinkType::Any, ())
}

#[hdk_extern]
fn delete_link(input: HeaderHash) -> ExternResult<HeaderHash> {
    hdk::prelude::delete_link(input)
}

#[hdk_extern]
fn get_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(base()?, None, None)
}

#[hdk_extern]
fn get_typed_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(base()?, Some(LinkTypes::Foo.into()), None)
}

#[hdk_extern]
fn get_baseless_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(baseless()?, None, None)
}

#[hdk_extern]
fn get_external_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(external()?, None, None)
}

#[hdk_extern]
fn get_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, None, None)
}

#[hdk_extern]
fn get_typed_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(base()?, Some(LinkTypes::Foo.into()), None)
}

#[hdk_extern]
fn get_back_links(_: ()) -> ExternResult<Vec<Link>> {
    hdk::prelude::get_links(target()?, None, None)
}

#[hdk_extern]
fn get_back_link_details(_: ()) -> ExternResult<LinkDetails> {
    hdk::prelude::get_link_details(target()?, None, None)
}

#[hdk_extern]
fn get_links_bidi(_: ()) -> ExternResult<Vec<Vec<Link>>> {
    HDK.with(|h| {
        h.borrow().get_links(vec![
            GetLinksInput::new(base()?, None, None),
            GetLinksInput::new(target()?, None, None),
        ])
    })
}

#[hdk_extern]
fn get_link_details_bidi(_: ()) -> ExternResult<Vec<LinkDetails>> {
    HDK.with(|h| {
        h.borrow().get_link_details(vec![
            GetLinksInput::new(base()?, None, None),
            GetLinksInput::new(target()?, None, None),
        ])
    })
}

#[hdk_extern]
fn delete_all_links(_: ()) -> ExternResult<()> {
    for link in hdk::prelude::get_links(base()?, None, None)? {
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
        hdk::prelude::create_link(
            parent.path_entry_hash()?.into(),
            path.path_entry_hash()?.into(),
            HdkLinkType::Any,
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
    Path::from("a").children()
}
