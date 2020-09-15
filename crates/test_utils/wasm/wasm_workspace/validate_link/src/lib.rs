use hdk3::prelude::*;

#[hdk_entry(id = "maybe_linkable")]
#[derive(Clone, Copy)]
enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

entry_defs![MaybeLinkable::entry_def()];

#[hdk_extern]
fn validate_link(
    validate_link_add_data: ValidateCreateLinkData,
) -> ExternResult<ValidateCreateLinkCallbackResult> {
    let base: MaybeLinkable = validate_link_add_data.base.try_into()?;
    let target: MaybeLinkable = validate_link_add_data.target.try_into()?;

    Ok(match base {
        MaybeLinkable::AlwaysLinkable => match target {
            MaybeLinkable::AlwaysLinkable => ValidateCreateLinkCallbackResult::Valid,
            _ => ValidateCreateLinkCallbackResult::Invalid("target never validates".to_string()),
        },
        _ => ValidateCreateLinkCallbackResult::Invalid("base never validates".to_string()),
    })
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry!(MaybeLinkable::AlwaysLinkable)?;
    create_entry!(MaybeLinkable::AlwaysLinkable)?;

    Ok(create_link!(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash
    )?)
}

#[hdk_extern]
fn add_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry!(MaybeLinkable::AlwaysLinkable)?;
    let never_linkable_entry_hash = hash_entry!(MaybeLinkable::NeverLinkable)?;

    create_entry!(MaybeLinkable::AlwaysLinkable)?;
    create_entry!(MaybeLinkable::NeverLinkable)?;

    Ok(create_link!(
        always_linkable_entry_hash,
        never_linkable_entry_hash
    )?)
}
