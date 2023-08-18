use std::collections::HashSet;
use std::collections::VecDeque;

use futures::FutureExt;
use kitsune_p2p_types::dependencies::ghost_actor::dependencies::must_future::MustBoxFuture;
use kitsune_p2p_types::{codec::Codec, KAgent};

use crate::error::{GossipRoundError, GossipRoundResult};
use crate::metrics::Metrics;
use crate::{codec::GossipMsg, PeerId};
use crate::{round, ArcNetCon};

use super::mux;

#[derive(Clone, Debug)]
pub(crate) enum HowToConnect {
    /// The connection handle and the url that this handle has been connected to.
    /// If the connection handle closes the url can change so we need to track it.
    Con(bool, String),
    Url(String),
}

// pub type ArcNetCon = Arc<dyn NetCon + 'static>;

pub type Incoming = (PeerId, GossipMsg);

pub struct StandardGossip {
    queue: VecDeque<Incoming>,
    mux: mux::GossipMux,
    local_agents: HashSet<KAgent>,
    /// Metrics that track remote node states and help guide
    /// the next node to gossip with.
    metrics: Metrics,
    queue_capacity: usize,
}

// pub enum Ax {
//     IncomingGossip(ArcNetCon, String, Box<[u8]>),
//     LocalAgentJoin(KAgent),
//     LocalAgentLeave(KAgent),
//     NewData,
//     Process,
// }

#[derive(PartialEq, Eq, derive_more::From)]
pub enum Fx {
    Process(Option<Incoming>),
    Noop,
}

#[stef::state]
impl stef::State for StandardGossip {
    type Action = Ax;
    type Effect = GossipRoundResult<Fx>;

    /// Decode and queue an incoming encoded gossip message
    pub fn incoming_gossip(
        &mut self,
        con: ArcNetCon,
        url: String,
        data: Box<[u8]>,
    ) -> GossipRoundResult<Fx> {
        if self.queue.len() >= self.queue_capacity {
            Err(GossipRoundError::Busy)
        } else {
            let (bytes, msg) = GossipMsg::decode_ref(&data)
                .map_err(|e| GossipRoundError::DecodeError(e.to_string()))?;
            self.queue.push_back((con.peer_id(), msg));
            Ok(Fx::Noop)
        }
    }

    /// Return the next message in the queue for processing.
    /// The intention is that the `Fx::Process` should be run through
    /// [`prepare_action`] and then passed to `GossipMux::transition`
    pub fn pop_incoming(&mut self) -> GossipRoundResult<Fx> {
        Ok(Fx::Process(self.queue.pop_front()))
    }

    // /// Pass a prepared action to the gossip multiplexer for processing, and
    // /// return any effects produced.
    // pub fn process(&mut self, (peer_id, ax): (PeerId, round::Ax)) -> GossipRoundResult<Fx> {
    //     Ok(Fx::Mux(self.mux.receive(peer_id, ax)))
    // }

    /// Add an agent to the local agent list
    pub fn local_agent_join(&mut self, agent: KAgent) -> GossipRoundResult<Fx> {
        self.local_agents.insert(agent);
        let fx = self.record_new_integrated_data();
        Ok(fx)
    }

    /// Remove an agent from the local agent list
    pub fn local_agent_leave(&mut self, agent: KAgent) -> GossipRoundResult<Fx> {
        self.local_agents.remove(&agent);
        Ok(Fx::Noop)
    }

    /// Signal to the gossip multiplexer that there is new data to gossip
    pub fn new_integrated_data(&mut self) -> GossipRoundResult<Fx> {
        Ok(self.record_new_integrated_data())
    }
}

impl StandardGossip {
    fn record_new_integrated_data(&mut self) -> Fx {
        self.metrics.record_force_initiate();
        Fx::Noop
    }

    /// Run the effects, which may result in GossipMux state transitions
    /// which produce more effects, which are also handled here.
    async fn handle_effect(&mut self, fx: Fx) -> GossipRoundResult<()> {
        match fx {
            Fx::Process(Some((peer_id, msg))) => {
                let ax = prepare_action(msg).await;
                let fx = self.mux.receive(peer_id, ax);
                for effect in fx {
                    match effect {
                        mux::Fx::Send(peer_id, msg) => todo!("send message"),
                        mux::Fx::FetchPoolPush(push) => todo!("add to fetch pool"),
                    }
                }
            }
            Fx::Process(None) | Fx::Noop => (),
        }
        Ok(())
    }
}

/// Do the necessary reads needed to construct a [`GossipRound`] Action
pub async fn prepare_action(msg: GossipMsg) -> round::Ax {
    use round::*;
    match msg {
        GossipMsg::Initiate(msg) => {
            let local_arcset = todo!();
            let local_agents = todo!();
            AxInitiate {
                msg,
                local_arcset,
                local_agents,
            }
            .into()
        }
        GossipMsg::Accept(_) => todo!(),
        GossipMsg::AgentDiff(_) => todo!(),
        GossipMsg::AgentData(_) => todo!(),
        GossipMsg::OpBloom(_) => todo!(),
        GossipMsg::OpRegions(_) => todo!(),
        GossipMsg::OpData(_) => todo!(),
        GossipMsg::OpBatchReceived(_) => todo!(),
        GossipMsg::Error(_) => todo!(),
        GossipMsg::Busy(_) => todo!(),
        GossipMsg::NoAgents(_) => todo!(),
        GossipMsg::AlreadyInProgress(_) => todo!(),
        GossipMsg::UnexpectedMessage(_) => todo!(),
    }
}
