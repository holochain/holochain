use hdk::prelude::*;

use crate::integrity::AgentsChain;
use crate::integrity::AgentsChainRec;
use crate::integrity::EntryTypes;
use crate::integrity::Something;

#[hdk_extern]
fn must_get_valid_record(action_hash: ActionHash) -> ExternResult<Record> {
    hdk::prelude::must_get_valid_record(action_hash)
}

#[hdk_extern]
fn must_get_action(action_hash: ActionHash) -> ExternResult<SignedActionHashed> {
    hdk::prelude::must_get_action(action_hash)
}

#[hdk_extern]
fn must_get_entry(entry_hash: EntryHash) -> ExternResult<EntryHashed> {
    hdk::prelude::must_get_entry(entry_hash)
}

#[hdk_extern]
fn call_must_get_agent_activity(
    input: (AgentPubKey, ChainFilter),
) -> ExternResult<Vec<RegisterAgentActivity>> {
    let (author, filter) = input;
    must_get_agent_activity(author, filter)
}

#[hdk_extern]
fn commit_something(something: Something) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::Something(something))
}

#[hdk_extern]
fn commit_require_agents_chain(input: (AgentPubKey, ChainFilter)) -> ExternResult<ActionHash> {
    let (author, filter) = input;
    create_entry(EntryTypes::AgentsChain(AgentsChain(author, filter)))
}

#[hdk_extern]
fn commit_require_agents_chain_recursive(
    input: (AgentPubKey, ActionHash),
) -> ExternResult<ActionHash> {
    let (author, chain_top) = input;
    create_entry(EntryTypes::AgentsChainRec(AgentsChainRec(
        author, chain_top,
    )))
}
