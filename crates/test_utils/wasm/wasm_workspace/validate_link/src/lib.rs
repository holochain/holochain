use hdk3::prelude::*;

#[hdk_entry(id = "maybe_linkable")]
#[derive(Clone, Copy)]
enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

entry_defs![MaybeLinkable::entry_def()];

#[hdk_extern]
fn validate_create_link(
    validate_create_link_data: ValidateCreateLinkData,
) -> ExternResult<ValidateLinkCallbackResult> {
    let base: MaybeLinkable = validate_create_link_data.base.try_into()?;
    let target: MaybeLinkable = validate_create_link_data.target.try_into()?;

    Ok(match base {
        MaybeLinkable::AlwaysLinkable => match target {
            MaybeLinkable::AlwaysLinkable => ValidateLinkCallbackResult::Valid,
            _ => ValidateLinkCallbackResult::Invalid("target never validates".to_string()),
        },
        _ => ValidateLinkCallbackResult::Invalid("base never validates".to_string()),
    })
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    add_valid_link_inner()
}

fn add_valid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = hash_entry(&MaybeLinkable::AlwaysLinkable)?;
    create_entry(&MaybeLinkable::AlwaysLinkable)?;

    Ok(create_link(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash,
        (),
    )?)
}

#[hdk_extern]
fn remove_valid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_valid_link_inner()?;
    Ok(delete_link(valid_link)?)
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

    Ok(create_link(
        never_linkable_entry_hash,
        always_linkable_entry_hash,
        (),
    )?)
}

#[hdk_extern]
fn remove_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_invalid_link_inner()?;
    Ok(delete_link(valid_link)?)
}

#[hdk_extern]
fn validate(_element: ValidateData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate_delete_link(
    validate_delete_link: ValidateDeleteLinkData,
) -> ExternResult<ValidateLinkCallbackResult> {
    let delete_link = validate_delete_link.delete_link;
    let base: Option<MaybeLinkable> =
        match get(delete_link.base_address.clone(), GetOptions::content())? {
            Some(b) => b.entry().to_app_option()?,
            None => {
                return Ok(ValidateLinkCallbackResult::UnresolvedDependencies(vec![
                    delete_link.base_address.into(),
                ]))
            }
        };
    let base = match base {
        Some(b) => b,
        None => {
            return Ok(ValidateLinkCallbackResult::Invalid(
                "Base of this entry is not MaybeLinkable".to_string(),
            ))
        }
    };
    Ok(match base {
        MaybeLinkable::AlwaysLinkable => ValidateLinkCallbackResult::Valid,
        _ => ValidateLinkCallbackResult::Invalid("base never validates".to_string()),
    })
}
