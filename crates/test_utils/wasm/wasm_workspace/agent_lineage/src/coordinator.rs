use hdi::{hdk_extern, prelude::ExternResult};
use hdk::prelude::*;

use crate::integrity::{EntryTypes, SomeEntry};

#[hdk_extern]
fn create_entry_if_same_agent(agent_pub_key: AgentPubKey) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::SomeEntry(SomeEntry {
        author: agent_pub_key,
        content: "some_text".to_string(),
    }))
}
