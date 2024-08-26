use hdi::{hdk_extern, prelude::ExternResult};
use hdk::{agent::get_agent_key_lineage, prelude::*};

use crate::integrity::{EntryTypes, SomeEntry};

#[hdk_extern]
fn create_entry_if_keys_of_same_lineage(
    agent_keys: (AgentPubKey, AgentPubKey),
) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::SomeEntry(SomeEntry {
        key_1: agent_keys.0,
        key_2: agent_keys.1,
        content: "some_text".to_string(),
    }))
}

#[hdk_extern]
fn get_lineage_of_agent_keys(agent_key: AgentPubKey) -> ExternResult<Vec<AgentPubKey>> {
    get_agent_key_lineage(agent_key)
}
