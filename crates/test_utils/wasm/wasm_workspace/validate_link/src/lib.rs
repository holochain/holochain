use hdk::prelude::*;

#[hdk_entry(id = "maybe_linkable")]
#[derive(Clone, Copy)]
enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

entry_defs![MaybeLinkable::entry_def()];

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        // This is a pretty pointless example as everything is valid.
        Op::RegisterCreateLink { base, target, .. } => {
            let base: MaybeLinkable = base.try_into()?;
            let target: MaybeLinkable = target.try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => match target {
                    MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                    _ => ValidateCallbackResult::Invalid("target never validates".to_string()),
                },
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        Op::RegisterDeleteLink { create_link, .. } => {
            let base: MaybeLinkable = must_get_entry(create_link.base_address)?.try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    add_valid_link_inner()
}

fn add_valid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    create_entry(&MaybeLinkable::AlwaysLinkable)?;

    create_link(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash,
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

    create_entry(&MaybeLinkable::AlwaysLinkable)?;
    create_entry(&MaybeLinkable::NeverLinkable)?;

    create_link(never_linkable_entry_hash, always_linkable_entry_hash, ())
}

#[hdk_extern]
fn remove_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_invalid_link_inner()?;
    delete_link(valid_link)
}
