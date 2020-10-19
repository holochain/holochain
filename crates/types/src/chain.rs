//! Types related to an agents for chain activity
use holo_hash::HeaderHash;
use holochain_zome_types::{
    element::SignedHeaderHashed, query::AgentActivity, query::ChainHead, query::ChainStatus, Header,
};

/// Helpers for constructing AgentActivity
pub trait AgentActivityExt {
    /// Create a valid chain activity from set of headers.
    /// The headers should come from an agents chain activity and
    /// be ordered in ascending order
    fn valid(headers: Vec<SignedHeaderHashed>) -> AgentActivity {
        let status = headers
            .last()
            .map(|chain_head| ChainStatus::Valid(head_from_header(chain_head.header())))
            .unwrap_or(ChainStatus::Empty);
        AgentActivity {
            activity: headers,
            status,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }

    /// Create a valid status without any activity
    fn valid_without_activity(header: &Header) -> AgentActivity {
        let status = ChainStatus::Valid(head_from_header(header));
        AgentActivity {
            activity: Vec::with_capacity(0),
            status,
            // TODO: Add the actual highest observed in a follow up PR
            highest_observed: None,
        }
    }

    /// Create an empty chain status
    fn empty() -> AgentActivity {
        AgentActivity {
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
