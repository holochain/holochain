use hdi::prelude::*;
use hdk::prelude::ChainFilter;

#[hdk_entry_helper]
#[derive(Clone)]
pub struct Something(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[hdk_entry_helper]
pub struct AgentsChain(pub AgentPubKey, pub ChainFilter);

#[hdk_entry_helper]
pub struct AgentsChainRec(pub AgentPubKey, pub ActionHash);

#[hdk_entry_defs]
#[unit_enum(EntryTypesUnit)]
pub enum EntryTypes {
    Something(Something),
    AgentsChain(AgentsChain),
    AgentsChainRec(AgentsChainRec),
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    match op.flattened::<_, ()>()? {
        FlatOp::StoreEntry(e) => match e {
            OpEntry::CreateEntry {
                app_entry: EntryTypes::AgentsChain(AgentsChain(author, filter)),
                ..
            } => {
                must_get_agent_activity(author, filter)?;
                return Ok(ValidateCallbackResult::Valid);
            }
            OpEntry::CreateEntry {
                app_entry: EntryTypes::AgentsChainRec(AgentsChainRec(author, chain_top)),
                ..
            } => {
                let mut filter = ChainFilter::new(chain_top).take(2);
                loop {
                    let chain = must_get_agent_activity(author.clone(), filter.clone())?;
                    if chain.len() > 2 {
                        return Ok(ValidateCallbackResult::Invalid(
                            "Filter returned greater than 2".to_string(),
                        ));
                    }
                    match chain.last() {
                        Some(op) => {
                            if op.action.action().action_seq() == 0 {
                                return Ok(ValidateCallbackResult::Valid);
                            } else {
                                filter =
                                    ChainFilter::new(op.action.action_address().clone()).take(2);
                            }
                        }
                        None => {
                            return Ok(ValidateCallbackResult::Invalid(
                                "Could not recurse to bottom of agents chain".to_string(),
                            ))
                        }
                    }
                }
            }
            _ => (),
        },
        _ => (),
    }

    Ok(ValidateCallbackResult::Valid)
}
