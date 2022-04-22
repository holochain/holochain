use holochain_deterministic_integrity::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);

#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_helper]
pub struct PrivMsg(pub String);

#[hdk_entry_defs]
pub enum EntryTypes {
    #[entry_def(required_validations = 5)]
    Post(Post),
    #[entry_def(required_validations = 5)]
    Msg(Msg),
    #[entry_def(required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}

#[cfg_attr(feature = "integrity", hdk_extern)]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    let this_zome = zome_info()?;
    if let Op::StoreEntry {
        header:
            SignedHashed {
                hashed: HoloHashed {
                    content: header, ..
                },
                ..
            },
        entry,
    } = op
    {
        let _: fn(Post) -> EntryTypes = EntryTypes::Post;
        if header
            .app_entry_type()
            .filter(|app_entry_type| {
                this_zome.matches_entry_def_id(
                    app_entry_type,
                    EntryTypes::variant_to_entry_def_id(EntryTypes::Post),
                )
            })
            .map_or(Ok(false), |_| {
                Post::try_from(entry).map(|post| &post.0 == "Banana")
            })?
        {
            return Ok(ValidateCallbackResult::Invalid("No Bananas!".to_string()));
        }
    }
    Ok(ValidateCallbackResult::Valid)
}
