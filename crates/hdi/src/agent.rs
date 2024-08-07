//! Calls related to agent keys.
//!
//! An agent can update their key. This is helpful in cases where a the private key of their key pair
//! has been leaked or becomes unusable in some other way. The agent key can be updated, which
//! invalidates the current key and generates a new key. Both the invalidated key and the new key
//! belong to the same agent. Keys of the same agent are called a key lineage.

use crate::prelude::*;
use holo_hash::AgentPubKey;

/// Check if agent key 2 belongs to the same agent as agent key 1, i. e. if they belong to the key
/// lineage that key 1 is part of.
pub fn is_same_agent(key_1: AgentPubKey, key_2: AgentPubKey) -> ExternResult<bool> {
    HDI.with(|h| h.borrow().is_same_agent(key_1, key_2))
}
