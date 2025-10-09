//! Types related to an agents for chain activity
use crate::warrant::WarrantOp;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::HasHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

mod chain_item;
pub use chain_item::*;

/// Intermediate data structure used during a `must_get_agent_activity` call.
/// Note that this is not the final return value of `must_get_agent_activity`.
#[derive(Debug, Clone, PartialEq, Eq, SerializedBytes, Serialize, Deserialize)]
pub enum MustGetAgentActivityResponse {
    /// The activity was found.
    Activity {
        /// The actions performed by the agent.
        activity: Vec<RegisterAgentActivity>,
        /// Any warrants issued to the agent for this activity.
        warrants: Vec<WarrantOp>,
    },
    /// The requested chain top was found, but the
    /// actions found within the filtered range
    /// were incomplete.
    IncompleteChain,
    /// The requested chain top was not found in the chain.
    ChainTopNotFound(ActionHash),
}

impl MustGetAgentActivityResponse {
    /// Constructor
    #[cfg(feature = "test_utils")]
    pub fn activity(activity: Vec<RegisterAgentActivity>) -> Self {
        Self::Activity {
            activity,
            warrants: vec![],
        }
    }
}
