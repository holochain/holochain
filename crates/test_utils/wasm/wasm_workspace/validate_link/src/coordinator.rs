use crate::integrity::*;
use hdk::prelude::*;

#[hdk_dependent_entry_types]
enum EntryZomes {
    IntegrityValidLink(EntryTypes),
}

#[hdk_dependent_link_types]
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
fn must_get_valid_commit(action_hash: ActionHash) -> ExternResult<Commit> {
    hdk::prelude::must_get_valid_commit(action_hash)
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<ActionHash> {
    add_valid_link_inner()
}

fn add_valid_link_inner() -> ExternResult<ActionHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::AlwaysLinkable))?;

    create_link(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash,
        LinkZomes::any(),
        (),
    )
}

#[hdk_extern]
fn remove_valid_link(_: ()) -> ExternResult<ActionHash> {
    let valid_link = add_valid_link_inner()?;
    delete_link(valid_link)
}

#[hdk_extern]
fn add_invalid_link(_: ()) -> ExternResult<ActionHash> {
    add_invalid_link_inner()
}

fn add_invalid_link_inner() -> ExternResult<ActionHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    let never_linkable_entry_hash = hash_entry(&MaybeLinkable::NeverLinkable)?;

    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::AlwaysLinkable))?;
    create_entry(&EntryZomes::maybe_linkable(MaybeLinkable::NeverLinkable))?;

    create_link(
        never_linkable_entry_hash,
        always_linkable_entry_hash,
        LinkZomes::any(),
        (),
    )
}

#[hdk_extern]
fn remove_invalid_link(_: ()) -> ExternResult<ActionHash> {
    let valid_link = add_invalid_link_inner()?;
    delete_link(valid_link)
}
