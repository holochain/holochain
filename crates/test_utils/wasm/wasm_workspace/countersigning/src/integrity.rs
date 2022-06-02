use holochain_deterministic_integrity::prelude::*;

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

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Thing(Thing),
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op {
        Op::StoreElement {
            element:
                Element {
                    signed_header,
                    entry: ElementEntry::Present(entry),
                },
        } => {
            match signed_header.header().entry_type().and_then(|et| match et {
                EntryType::App(AppEntryType { id, .. }) => Some(id),
                _ => None,
            }) {
                Some(id) => match EntryTypes::try_from_global_type(*id, &entry) {
                    Ok(Some(EntryTypes::Thing(thing))) => Ok(thing.into()),
                    Ok(None) => Ok(ValidateCallbackResult::Valid),
                    Err(WasmError::Deserialize(_)) => Ok(ValidateCallbackResult::Invalid(
                        "Failed to deserialize entry".to_string(),
                    )),
                    Err(e) => Err(e),
                },
                None => Ok(ValidateCallbackResult::Valid),
            }
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
