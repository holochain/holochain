use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);

#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_helper]
pub struct PrivMsg(pub String);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    #[entry_def(required_validations = 5)]
    Post(Post),
    #[entry_def(required_validations = 5)]
    Msg(Msg),
    #[entry_def(required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}

#[hdk_link_types]
pub enum LinkTypes {
    Post,
}

#[cfg_attr(feature = "integrity", hdk_extern)]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    if let Op::StoreEntry(StoreEntry {
        action:
            SignedHashed {
                hashed: HoloHashed {
                    content: action, ..
                },
                ..
            },
        entry,
    }) = op
    {
        action
            .app_entry_def()
            .map(|AppEntryDef { entry_index, zome_index, .. }| (zome_index, entry_index))
            .map_or(Ok(ValidateCallbackResult::Valid), |(zome_index, entry_index)| {
                match EntryTypes::deserialize_from_type(*zome_index, *entry_index, &entry)? {
                    Some(EntryTypes::Post(post)) if post.0 == "Banana" => {
                        Ok(ValidateCallbackResult::Invalid("No Bananas!".to_string()))
                    }
                    _ => Ok(ValidateCallbackResult::Valid),
                }
            })
    } else {
        Ok(ValidateCallbackResult::Valid)
    }
}
