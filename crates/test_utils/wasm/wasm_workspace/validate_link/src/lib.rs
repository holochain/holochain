use hdk3::prelude::*;

const MAYBE_LINKABLE_ID: &str = "maybe_linkable";
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Clone, Copy)]
enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

entry_def!(MaybeLinkable EntryDef {
    id: MAYBE_LINKABLE_ID.into(),
    ..Default::default()
});

entry_defs![MaybeLinkable::entry_def()];

#[hdk(extern)]
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

#[hdk(extern)]
fn add_valid_link(_: ()) -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = entry_hash!(MaybeLinkable::AlwaysLinkable)?;
    commit_entry!(MaybeLinkable::AlwaysLinkable)?;

    Ok(link_entries!(
        always_linkable_entry_hash.clone(),
        always_linkable_entry_hash
    )?)
}

#[hdk(extern)]
fn add_invalid_link(_: ()) -> ExternResult<HeaderHash> {
    let always_linkable_entry_hash = entry_hash!(MaybeLinkable::AlwaysLinkable)?;
    let never_linkable_entry_hash = entry_hash!(MaybeLinkable::NeverLinkable)?;

    commit_entry!(MaybeLinkable::AlwaysLinkable)?;
    commit_entry!(MaybeLinkable::NeverLinkable)?;

    Ok(link_entries!(
        always_linkable_entry_hash,
        never_linkable_entry_hash
    )?)
}
