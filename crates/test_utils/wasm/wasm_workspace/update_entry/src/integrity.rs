use hdi::prelude::*;

#[hdk_entry_helper]
pub struct Post(pub String);

#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    #[entry_def(required_validations = 5)]
    Post(Post),
    #[entry_def(required_validations = 5)]
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
        Op::StoreEntry { action, entry } => match action.hashed.app_entry_type() {
            Some(AppEntryType {
                id: entry_def_index,
                zome_id,
                ..
            }) => match EntryTypes::deserialize_from_type(*zome_id, *entry_def_index, &entry)? {
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
