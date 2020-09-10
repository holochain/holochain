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
    validate_link_add_data: ValidateLinkAddData,
) -> ExternResult<ValidateLinkAddCallbackResult> {
    let base: MaybeLinkable = validate_link_add_data.base.try_into()?;
    let target: MaybeLinkable = validate_link_add_data.target.try_into()?;

    Ok(match base {
        MaybeLinkable::AlwaysLinkable => match target {
            MaybeLinkable::AlwaysLinkable => ValidateLinkAddCallbackResult::Valid,
            _ => ValidateLinkAddCallbackResult::Invalid("target never validates".to_string()),
        },
        _ => ValidateLinkAddCallbackResult::Invalid("base never validates".to_string()),
    })
}

#[hdk_extern]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    add_valid_link_inner()
}

fn add_valid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = entry_hash!(MaybeLinkable::AlwaysLinkable)?;
    commit_entry!(MaybeLinkable::AlwaysLinkable)?;

    Ok(link_entries!(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash
    )?)
}

#[hdk_extern]
fn remove_valid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_valid_link_inner()?;
    Ok(remove_link!(valid_link)?)
}

#[hdk_extern]
fn add_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    add_invalid_link_inner()
}

fn add_invalid_link_inner() -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = entry_hash!(MaybeLinkable::AlwaysLinkable)?;
    let never_linkable_entry_hash = entry_hash!(MaybeLinkable::NeverLinkable)?;

    commit_entry!(MaybeLinkable::AlwaysLinkable)?;
    commit_entry!(MaybeLinkable::NeverLinkable)?;

    Ok(link_entries!(
        always_linkable_entry_hash,
        never_linkable_entry_hash
    )?)
}

#[hdk_extern]
fn remove_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let valid_link = add_invalid_link_inner()?;
    Ok(remove_link!(valid_link)?)
}

#[hdk_extern]
fn validate(_element: Element) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate_remove_link(element: Element) -> ExternResult<ValidateCallbackResult> {
    match (element.into_inner().0.into_inner().0).0 {
        Header::LinkRemove(link_remove) => {
            let base: Option<MaybeLinkable> = match get!(link_remove.base_address.clone())? {
                Some(b) => b.entry().to_app_option()?,
                None => {
                    return Ok(ValidateCallbackResult::UnresolvedDependencies(vec![
                        link_remove.base_address.clone(),
                    ]))
                }
            };
            let base = match base {
                Some(b) => b,
                None => {
                    return Ok(ValidateCallbackResult::Invalid(
                        "Base of this entry is not MaybeLinkable".to_string(),
                    ))
                }
            };
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        _ => Ok(ValidateCallbackResult::Invalid(
            "Not a LinkRemove header".to_string(),
        )),
    }
}
