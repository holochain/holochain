//! Types related to an agents for chain activity
use holo_hash::{AgentPubKey, HeaderHash};
use holochain_zome_types::{
    element::SignedHeaderHashed, query::AgentActivity, query::ChainHead, query::ChainStatus, Header,
};

/// Helpers for constructing AgentActivity
pub trait AgentActivityExt {
    /// Create a valid chain activity from set of headers.
    /// The headers should come from an agents chain activity and
    /// be ordered in ascending order
    fn valid(headers: Vec<SignedHeaderHashed>, agent: AgentPubKey) -> AgentActivity {
        let status = headers
            .last()
            .map(|chain_head| ChainStatus::Valid(head_from_header(chain_head.header())))
            .unwrap_or(ChainStatus::Empty);
        AgentActivity {
            agent,
            activity: headers,
            status,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }

    /// Create a valid status without any activity
    fn valid_without_activity(chain_head: ChainHead, agent: AgentPubKey) -> AgentActivity {
        let status = ChainStatus::Valid(chain_head);
        AgentActivity {
            agent,
            activity: Vec::with_capacity(0),
            status,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }

    /// Create an empty chain status
    fn empty(agent: AgentPubKey) -> AgentActivity {
        AgentActivity {
            agent,
            activity: Vec::with_capacity(0),
            status: ChainStatus::Empty,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }
}

impl AgentActivityExt for AgentActivity {}

fn head_from_header(h: &Header) -> ChainHead {
    let hash = HeaderHash::with_data_sync(h);
    ChainHead {
        header_seq: h.header_seq(),
        hash,
    }
}
