use kitsune_p2p_fetch::FetchPoolPush;
use stef::{ParamState, State};

use crate::{codec::GossipMsg, round::AxInitiate, PeerId};

use super::round::{self, GossipRound};
use std::collections::HashMap;

/// The Gossip (De)Multiplexer manages state for each ongoing gossip round
/// with other nodes. It finds new opportunities to start rounds with other
/// nodes, and processes incoming messages from other nodes, routing them
/// to the correct [`GossipRound`], and passing the side effects back to the
/// gossip module so that new messages get sent and received data gets persisted.
pub struct GossipMux {
    rounds: HashMap<PeerId, GossipRound>,
}

#[stef::state]
impl stef::State for GossipMux {
    type Action = Ax;
    type Effect = Vec<Fx>;

    pub fn try_initiate(&mut self) -> Vec<Fx> {
        todo!()
    }

    pub fn receive(&mut self, peer_id: PeerId, ax: round::Ax) -> Vec<Fx> {
        let fx = if let Some(round) = self.rounds.get_mut(&peer_id) {
            round.transition(ax)
        } else if let round::Ax::Initiate(AxInitiate { msg, .. }) = ax {
            let params = round::GossipRoundParams::new(msg.plan, false);
            let (mut round, fx) = GossipRound::new(params);
            self.rounds.insert(peer_id.clone(), round);
            vec![fx]
        } else {
            vec![]
        };

        // map inner Effect into outer Effect type
        fx.into_iter()
            .map(|fx| match fx {
                round::Fx::Send(fx) => Fx::Send(peer_id.clone(), fx),
                _ => todo!("handle other fx"),
            })
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq, derive_more::From)]
#[must_use]
pub enum Fx {
    /// Send a message to a peer
    Send(PeerId, round::FxSend),
    /// Add an op hash to the FetchPool
    FetchPoolPush(FetchPoolPush),
}
