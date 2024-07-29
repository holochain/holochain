//! Calls related to agent keys.

use crate::prelude::*;
use holo_hash::AgentPubKey;

/// Check if agent key 2 belongs to the same agent as agent key 1.
pub fn is_same_agent(key_1: AgentPubKey, key_2: AgentPubKey) -> ExternResult<bool> {
    HDI.with(|h| h.borrow().is_same_agent(key_1, key_2))
}
