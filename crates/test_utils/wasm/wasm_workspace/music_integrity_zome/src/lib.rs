use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Artist(pub String);

#[hdk_entry_helper]
pub struct Song(pub String);

#[hdk_entry_defs]
#[unit_enum(UnitMusicTypes)]
pub enum MusicTypes {
    Artist(Artist),
    Song(Song),
}

#[hdk_link_types]
pub enum LinkTypes {
    Produced,
}

#[hdk_extern]
/// This validation callback only allows storing of entries
/// that successfully match and deserialize to the entry types
/// or link types defined within this zome.
/// All other ops are allowed.
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.to_type()? {
        OpType::StoreEntry(OpEntry::CreateEntry { entry_type, .. }) => match entry_type {
            MusicTypes::Artist(_) => Ok(ValidateCallbackResult::Valid),
            MusicTypes::Song(_) => Ok(ValidateCallbackResult::Valid),
        },
        OpType::RegisterCreateLink { link_type, .. } => match link_type {
            LinkTypes::Produced => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
