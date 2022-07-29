use hdi::prelude::*;

#[derive(Clone, Copy)]
#[hdk_entry_helper]
pub enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    MaybeLinkable(MaybeLinkable),
}

#[hdk_link_types]
pub enum LinkTypes {
    Any,
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        // This is a pretty pointless example as everything is valid.
        Op::RegisterCreateLink(RegisterCreateLink {  create_link  }) => {
            let base: MaybeLinkable =
                must_get_entry(create_link.hashed.content.base_address.into())?.try_into()?;
            let target: MaybeLinkable =
                must_get_entry(create_link.hashed.content.target_address.into())?.try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => match target {
                    MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                    _ => ValidateCallbackResult::Invalid("target never validates".to_string()),
                },
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        Op::RegisterDeleteLink(RegisterDeleteLink {  create_link, ..  }) => {
            let base: MaybeLinkable =
                must_get_entry(create_link.base_address.into())?.try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
