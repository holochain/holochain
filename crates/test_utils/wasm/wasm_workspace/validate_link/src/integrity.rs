use hdi::prelude::*;

#[derive(Clone, Copy)]
#[hdk_entry_helper]
pub enum MaybeLinkable {
    AlwaysLinkable,
    NeverLinkable,
}

#[hdk_entry_types]
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
    match op.flattened::<EntryTypes, LinkTypes>()? {
        // This is a pretty pointless example as everything is valid.
        FlatOp::Link(link @ OpLink::CreateLink { .. }) => {
            let base_address = link.base_address().clone();
            let target_address = link.target_address().clone();
            let base: MaybeLinkable =
                must_get_entry(base_address.into_entry_hash().expect("must be entry hash"))?
                    .try_into()?;
            let target: MaybeLinkable = must_get_entry(
                target_address
                    .into_entry_hash()
                    .expect("must be entry hash"),
            )?
            .try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => match target {
                    MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                    _ => ValidateCallbackResult::Invalid("target never validates".to_string()),
                },
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        FlatOp::Link(link @ OpLink::DeleteLink { .. }) => {
            let base_address = link.base_address().clone();
            let base: MaybeLinkable =
                must_get_entry(base_address.into_entry_hash().expect("must be entry hash"))?
                    .try_into()?;
            Ok(match base {
                MaybeLinkable::AlwaysLinkable => ValidateCallbackResult::Valid,
                _ => ValidateCallbackResult::Invalid("base never validates".to_string()),
            })
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
