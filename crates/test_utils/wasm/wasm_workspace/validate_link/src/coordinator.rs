use crate::integrity::*;
use hdk::prelude::*;

#[hdk_entry_zomes]
enum EntryZomes {
    IntegrityValidLink(EntryTypes),
}

#[hdk_link_zomes]
enum LinkZomes {
    IntegrityValidLink(LinkTypes),
}

impl EntryZomes {
    fn maybe_linkable(l: MaybeLinkable) -> Self {
        Self::IntegrityValidLink(EntryTypes::MaybeLinkable(l))
    }
}

impl LinkZomes {
    fn any() -> Self {
        Self::IntegrityValidLink(LinkTypes::Any)
    }
}

#[hdk_extern]
fn must_get_valid_element(header_hash: HeaderHash) -> ExternResult<Element> {
    hdk::prelude::must_get_valid_element(header_hash)
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    add_valid_link_inner()
}

fn add_valid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::AlwaysLinkable))?;

    create_link(
        always_linkable_entry_hash.clone().into(),
        always_linkable_entry_hash.into(),
        LinkZomes::any(),
        (),
    )
}

#[hdk_extern]
fn remove_valid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_valid_link_inner()?;
    delete_link(valid_link)
}

#[hdk_extern]
fn add_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    add_invalid_link_inner()
}

fn add_invalid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    let never_linkable_entry_hash = hash_entry(&MaybeLinkable::NeverLinkable)?;

    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::AlwaysLinkable))?;
    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::NeverLinkable))?;

    create_link(
        never_linkable_entry_hash.into(),
        always_linkable_entry_hash.into(),
        LinkZomes::any(),
        (),
    )
}

#[hdk_extern]
fn remove_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_invalid_link_inner()?;
    delete_link(valid_link)
}
