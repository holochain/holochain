use hdi::prelude::*;

#[hdi_entry_helper]
#[derive(Clone, PartialEq)]
pub struct Thing {
    pub content: u32,
}

#[derive(Serialize, Deserialize)]
#[hdi_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Thing(Thing),
}

fn validate_create_thing(action: EntryCreationAction) -> ExternResult<ValidateCallbackResult> {
    let author = action.author().clone();
    if let EntryCreationAction::Create(action) = action {
        let action_hash = hash_action(action.into_action().clone())?;
        let filter = ChainFilter::new(action_hash);
        let result = must_get_agent_activity(author.clone(), filter)?;
        debug!("Agent Activity Count: {}", result.len());
    }
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
pub fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, ()>()? {
        FlatOp::StoreEntry(store_entry) => match store_entry {
            OpEntry::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::Thing(_) => validate_create_thing(EntryCreationAction::Create(action)),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        FlatOp::StoreRecord(store_record) => match store_record {
            OpRecord::CreateEntry { app_entry, action } => match app_entry {
                EntryTypes::Thing(_) => validate_create_thing(EntryCreationAction::Create(action)),
            },
            _ => Ok(ValidateCallbackResult::Valid),
        },
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
