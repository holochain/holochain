use hdi::prelude::*;

#[hdk_entry_helper]
pub enum Thing {
    Valid,
    Invalid,
}

impl From<Thing> for ValidateCallbackResult {
    fn from(thing: Thing) -> Self {
        match thing {
            Thing::Valid => ValidateCallbackResult::Valid,
            Thing::Invalid => ValidateCallbackResult::Invalid("never valid".to_string()),
        }
    }
}

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Thing(Thing),
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreRecord(StoreRecord {
            record:
                Record {
                    signed_action,
                    entry: RecordEntry::Present(entry),
                },
        }) => {
            match signed_action.action().entry_type().and_then(|entry_type| match entry_type {
                EntryType::App(AppEntryDef { entry_index, zome_index, .. }) => Some((zome_index, entry_index)),
                _ => None,
            }) {
                Some((zome_index, entry_index)) => {
                    match EntryTypes::deserialize_from_type(*zome_index, *entry_index, &entry) {
                        Ok(Some(EntryTypes::Thing(thing))) => Ok(thing.into()),
                        Ok(None) => Ok(ValidateCallbackResult::Valid),
                        Err(WasmError {
                            error: WasmErrorInner::Deserialize(_),
                            ..
                        }) => Ok(ValidateCallbackResult::Invalid(
                            "Failed to deserialize entry".to_string(),
                        )),
                        Err(e) => Err(e),
                    }
                }
                None => Ok(ValidateCallbackResult::Valid),
            }
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
