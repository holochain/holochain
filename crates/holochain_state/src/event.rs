use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Event {
    /// The node has integrated an op authored by someone else
    Integrated { op: DhtOpHash },
    /// The node has app validated an op authored by someone else
    AppValidated { op: DhtOpHash },
    /// The node has sys validated an op authored by someone else
    SysValidated { op: DhtOpHash },

    /// TODO: handle a missing app validation dep
    MissingAppValDep {
        op: DhtOpHash,
        deps: Vec<AnyDhtHash>,
    },
    /// The node has fetched an op after hearing about the hash via publish or gossip
    Fetched { op: DhtOpHash },
    /// The node has published or gossiped this at least once, to somebody
    SentHash {
        op: DhtOpHash,
        method: TransferMethod,
    },
    /// The node has received an op hash via publish or gossip
    ReceivedHash {
        op: DhtOpHash,
        method: TransferMethod,
    },
    /// The node has authored this op, including validation and integration
    Authored { op: DhtOp },
    /// An agent has joined the network
    AgentJoined { agent: AgentPubKey },
}
