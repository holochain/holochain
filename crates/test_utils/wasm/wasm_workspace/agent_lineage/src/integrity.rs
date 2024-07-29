use hdi::{agent::is_same_agent, prelude::OpEntry};
use hdk::prelude::*;

#[hdk_entry_helper]
pub(crate) struct SomeEntry {
    pub content: String,
    pub author: AgentPubKey,
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
            action,
        }) => {
            tracing::error!(
                "here the action author key is {:?} and the entry author key is {:?}",
                action.author,
                some_entry.author
            );
            let isa = is_same_agent(action.author.clone(), some_entry.author);
            tracing::error!("is same agent is {isa:?}");
            let isa = isa?;
            if let true = isa {
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
