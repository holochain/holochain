//! Some events that we wish to eventually support, but don't just yet.

use crate::prelude::*;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum UnsupportedEvent {
    /// TODO: handle a missing app validation dep
    MissingAppValDep {
        op: DhtOpHash,
        deps: Vec<AnyDhtHash>,
    },

    /// The node has published or gossiped this at least once, to somebody
    ///
    /// We won't support this currently because there is no persistent
    /// state corresponding to the sending of an op hash.
    SentHash {
        op: DhtOpHash,
        method: TransferMethod,
    },

    /// The node has received an op hash via publish or gossip.
    ///
    /// We can't support this currently because the fetch pool is ephemeral
    /// and we don't have a persistent place to record that we've received an op hash.
    ReceivedHash {
        op: DhtOpHash,
        method: TransferMethod,
    },

    /// An agent has joined the network
    AgentJoined { agent: AgentPubKey },
}
