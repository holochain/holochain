use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);

#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    #[entry_type(required_validations = 5)]
    Post(Post),
    #[entry_type(required_validations = 5)]
    Msg(Msg),
}

pub fn post() -> EntryTypes {
    EntryTypes::Post(Post("foo".into()))
}

pub fn msg() -> EntryTypes {
    EntryTypes::Msg(Msg("hi".into()))
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreEntry(StoreEntry { action, entry }) => match action.hashed.app_entry_def() {
            Some(AppEntryDef {
                entry_index: entry_def_index,
                zome_index,
                ..
            }) => match EntryTypes::deserialize_from_type(*zome_index, *entry_def_index, &entry)? {
                Some(EntryTypes::Post(Post(p))) => {
                    if p != "foo" {
                        return Ok(ValidateCallbackResult::Invalid("because".into()));
                    }
                }
                Some(EntryTypes::Msg(_)) => (),
                None => (),
            },
            _ => (),
        },
        _ => (),
    }
    Ok(ValidateCallbackResult::Valid)
}
