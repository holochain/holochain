//! Types related to an agents for chain activity
use crate::activity::AgentActivity;
use crate::activity::ChainItems;
use holo_hash::AgentPubKey;
use holochain_zome_types::ChainStatus;

/// Helpers for constructing AgentActivity
pub trait AgentActivityExt {
    /// Create an empty chain status
    fn empty<T>(agent: &AgentPubKey) -> AgentActivity<T> {
        AgentActivity {
            agent: agent.clone(),
            valid_activity: ChainItems::NotRequested,
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Empty,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }
}

impl AgentActivityExt for AgentActivity {}
