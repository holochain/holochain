use hdi::prelude::*;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Thing {
    pub content: u32,
}

#[derive(Serialize, Deserialize)]
#[hdk_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Thing(Thing),
}

fn validate_create_thing(action: Action) -> ExternResult<ValidateCallbackResult> {
    let author = action.author().clone();
    let action_hash = hash_action(action)?;
    let filter = ChainFilter::new(action_hash);
    let result = must_get_agent_activity(author.clone(), filter)?;
    debug!("Agent Activity Count: {}", result.len());
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, ()>()? {
        FlatOp::StoreEntry(OpEntry::CreateEntry { app_entry, action }) => match app_entry {
            EntryTypes::Thing(_) => validate_create_thing(action),
        },
        FlatOp::StoreRecord(OpRecord::CreateEntry { app_entry, action }) => match app_entry {
            EntryTypes::Thing(_) => validate_create_thing(action),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
