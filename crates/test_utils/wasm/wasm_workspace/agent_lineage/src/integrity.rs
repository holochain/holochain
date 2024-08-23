use hdi::{agent::is_same_agent, prelude::OpEntry};
use hdk::prelude::*;

#[hdk_entry_helper]
pub(crate) struct SomeEntry {
    pub content: String,
    pub key_1: AgentPubKey,
    pub key_2: AgentPubKey,
}

#[hdk_entry_types]
#[unit_enum(EntryTypesUnit)]
pub(crate) enum EntryTypes {
    SomeEntry(SomeEntry),
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<EntryTypes, ()>()? {
        hdi::prelude::FlatOp::StoreEntry(OpEntry::CreateEntry {
            app_entry: EntryTypes::SomeEntry(some_entry),
            ..
        }) => {
            if is_same_agent(some_entry.key_1, some_entry.key_2)? {
                Ok(ValidateCallbackResult::Valid)
            } else {
                Ok(ValidateCallbackResult::Invalid(
                    "agent key is not of same lineage".to_string(),
                ))
            }
        }
        _ => Ok(ValidateCallbackResult::Valid),
    }
}
