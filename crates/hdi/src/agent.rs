//! Calls related to agent keys.
//!
//! An agent can update their key. This is helpful in cases where a the private key of their key pair
//! has been leaked or becomes unusable in some other way. The agent key can be updated, which
//! invalidates the current key and generates a new key. Both the invalidated key and the new key
//! belong to the same agent. Keys of the same agent are called a key lineage.

// Tests are located under conductor::conductor::tests::agent_lineage.

/// Check if agent key 2 belongs to the same agent as agent key 1, i. e. if they belong to the key
/// lineage that key 1 is part of.
#[cfg(feature = "unstable-functions")]
pub fn is_same_agent(key_1: holo_hash::AgentPubKey, key_2: holo_hash::AgentPubKey) -> crate::prelude::ExternResult<bool> {
    crate::hdi::HDI.with(|h| h.borrow().is_same_agent(key_1, key_2))
}
