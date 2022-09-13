//! Tools for simulating a network around real holochain nodes.
//! This is a very early prototype and subject to change.

use fixt::prelude::Distribution;
use futures::stream::Stream;
use kitsune_p2p::actor::BroadcastTo;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Range;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::types::AgentPubKeyExt;
use crate::types::DnaHashExt;
use futures::StreamExt;
use holo_hash::{AgentPubKey, DnaHash};
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::dependencies::kitsune_p2p_proxy::ProxyUrl;
use kitsune_p2p::test_util::mock_network::to_kitsune_channel;
use kitsune_p2p::test_util::mock_network::KitsuneMock;
use kitsune_p2p::test_util::mock_network::ToKitsuneMockChannelRx;
use kitsune_p2p::test_util::mock_network::ToKitsuneMockChannelTx;
use kitsune_p2p::test_util::mock_network::{FromKitsuneMockChannelTx, KitsuneMockRespond};
use kitsune_p2p::wire as kwire;
use kitsune_p2p::GossipModuleType;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::tx2::MsgId;
use kitsune_p2p_types::Tx2Cert;

static MSG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_msg_id() -> MsgId {
    MsgId::new(MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// A channel between the simulated network and the set of real
/// holochain nodes.
pub struct HolochainP2pMockChannel {
    address_map: HashMap<AgentPubKey, (Tx2Cert, TxUrl)>,
    from_kitsune: Pin<Box<dyn Stream<Item = KitsuneMock> + Send + Sync + 'static>>,
    to_kitsune: ToKitsuneMockChannelTx,
}

#[derive(Clone)]
/// The conditions the simulated network will have.
pub struct MockScenario {
    /// Percentage of messages that will be dropped.
    pub percent_drop_msg: f32,
    /// Percentage of nodes that will not respond for the whole test.
    pub percent_offline: f32,
    /// The range of time inbound messages will be delayed.
    pub inbound_delay_range: Range<Duration>,
    /// The range of time outbound messages will be delayed.
    pub outbound_delay_range: Range<Duration>,
}

impl Default for MockScenario {
    fn default() -> Self {
        Self {
            percent_drop_msg: 0.0,
            percent_offline: 0.0,
            inbound_delay_range: Duration::from_millis(0)..Duration::from_millis(0),
            outbound_delay_range: Duration::from_millis(0)..Duration::from_millis(0),
        }
    }
}

#[derive(Debug)]
/// When a message is received or sent it needs to be
/// associated with a simulated connection.
/// This type wraps the message with simulated agent that
/// is associated with the connection.
pub struct AddressedHolochainP2pMockMsg {
    /// The network message.
    pub msg: HolochainP2pMockMsg,
    /// The simulated agent associated with this connection.
    pub agent: AgentPubKey,
}

#[derive(Debug)]
/// The network message for communicating with holochain.
/// This collapses a few levels of wire messages to make it
/// easier to build a network simulation.
/// Some wire messages are handled entirely within kitsune
/// which means there would be no way to simulate things like gossip
/// without this type.
pub enum HolochainP2pMockMsg {
    /// The holochain p2p wire messages.
    /// These can be either notifies or requests (Calls).
    Wire {
        /// The agent this message is addressed to.
        to_agent: AgentPubKey,
        /// The dna space this message is for.
        dna: DnaHash,
        /// The actual wire message.
        msg: super::wire::WireMessage,
    },
    /// A response to a request (Call).
    CallResp(kitsune_p2p::wire::WireData),
    /// A request from kitsune for a peers agent info.
    PeerGet(kitsune_p2p::wire::PeerGet),
    /// A response to a peer get.
    PeerGetResp(kitsune_p2p::wire::PeerGetResp),
    /// A query for multiple peers agent info.
    PeerQuery(kitsune_p2p::wire::PeerQuery),
    /// A response to peer query.
    PeerQueryResp(kitsune_p2p::wire::PeerQueryResp),
    /// A gossip protocol message.
    /// These messages are all notifies and not request.
    Gossip {
        /// The dna space this gossip is about.
        dna: DnaHash,
        /// The type of gossip module this message is for.
        module: GossipModuleType,
        /// The actual gossip wire protocol.
        gossip: GossipProtocol,
    },
    /// MetricExchange
    MetricExchange(kitsune_p2p::wire::MetricExchange),
    /// Agent info publish.
    PublishedAgentInfo {
        /// The agent this message is addressed to.
        to_agent: AgentPubKey,
        /// The dna space this message is for.
        dna: DnaHash,
        /// The agent info that is published.
        info: AgentInfoSigned,
    },
    /// Aan error has occurred.
    Failure(String),
}

#[derive(Debug)]
/// The type of protocol for the gossip wire message.
pub enum GossipProtocol {
    /// Simple bloom gossip wire protocol.
    // TODO: Implement this.
    Simple,
    /// Sharded gossip wire protocol.
    Sharded(kitsune_p2p::gossip::sharded_gossip::ShardedGossipWire),
}

/// This type allows a response to be sent to
/// a request message such as a call.
pub struct HolochainP2pMockRespond {
    respond: KitsuneMockRespond,
}

impl HolochainP2pMockRespond {
    /// Respond to a message request.
    pub fn respond(self, msg: HolochainP2pMockMsg) {
        self.respond.respond(msg.into_wire_msg());
    }
}

impl HolochainP2pMockChannel {
    /// Create a new mock simulated network channel.
    /// The peer data is the simulated nodes.
    /// The buffer is the amount of messages that can be buffered.
    /// The scenario sets up the why this simulated network will behave.
    pub fn channel(
        peer_data: Vec<AgentInfoSigned>,
        buffer: usize,
        scenario: MockScenario,
    ) -> (
        FromKitsuneMockChannelTx,
        ToKitsuneMockChannelRx,
        HolochainP2pMockChannel,
    ) {
        let address_map: HashMap<_, _> = peer_data
            .into_iter()
            .map(|info| {
                let agent = holo_hash::AgentPubKey::from_kitsune(&info.agent);
                let url = info.url_list.get(0).cloned().unwrap();
                let cert = Tx2Cert::from(ProxyUrl::from_full(url.as_str()).unwrap().digest());
                (agent, (cert, url))
            })
            .collect();
        let offline_nodes: HashSet<_> = {
            let mut rng = rand::thread_rng();
            address_map
                .values()
                .filter(|_| {
                    let offline = rand::distributions::Uniform::from(0.0..1.0);
                    offline.sample(&mut rng) <= scenario.percent_offline
                })
                .map(|(cert, _)| cert.clone())
                .collect()
        };
        let offline_nodes = Arc::new(offline_nodes);
        let (from_kitsune_tx, from_kitsune_rx) = tokio::sync::mpsc::channel(buffer);
        let (to_kitsune_tx, to_kitsune_rx) = to_kitsune_channel(buffer);
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);

        let stream = tokio_stream::wrappers::ReceiverStream::new(from_kitsune_rx)
            .map({
                let scenario = scenario.clone();
                move |t: KitsuneMock| {
                    let scenario = scenario.clone();
                    let offline_nodes = offline_nodes.clone();
                    async move {
                        let (delay, keep) = {
                            let mut rng = rand::thread_rng();
                            let delay = if scenario.inbound_delay_range.is_empty() {
                                Duration::from_millis(0)
                            } else {
                                let delay = rand::distributions::Uniform::from(
                                    scenario.inbound_delay_range,
                                );
                                delay.sample(&mut rng)
                            };
                            let drop = rand::distributions::Uniform::from(0.0..1.0);
                            let keep = drop.sample(&mut rng) > scenario.percent_drop_msg;
                            (delay, keep)
                        };
                        tokio::time::sleep(delay).await;
                        keep.then(|| t)
                            .filter(|m| !offline_nodes.contains(m.cert()))
                    }
                }
            })
            .buffer_unordered(10)
            .filter_map(|t| async move { t });
        let from_kitsune = Box::pin(stream);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx).for_each_concurrent(10, {
            move |msg| {
                let scenario = scenario.clone();
                let to_kitsune_tx = to_kitsune_tx.clone();
                async move {
                    let scenario = scenario.clone();
                    let (delay, keep) = {
                        let mut rng = rand::thread_rng();
                        let delay = if scenario.outbound_delay_range.is_empty() {
                            Duration::from_millis(0)
                        } else {
                            let delay =
                                rand::distributions::Uniform::from(scenario.outbound_delay_range);
                            delay.sample(&mut rng)
                        };
                        let drop = rand::distributions::Uniform::from(0.0..1.0);
                        let keep = drop.sample(&mut rng) > scenario.percent_drop_msg;
                        (delay, keep)
                    };
                    tokio::time::sleep(delay).await;
                    if keep {
                        to_kitsune_tx.send(msg).await.unwrap();
                    }
                }
            }
        });
        tokio::spawn(async move {
            stream.await;
        });

        (
            from_kitsune_tx,
            to_kitsune_rx,
            Self {
                address_map,
                from_kitsune,
                to_kitsune: tx,
            },
        )
    }

    /// Await the next message from the real nodes.
    /// Note that all messages are routed through this call so blocking
    /// on the loop that calls next will slow down all simulated node response times.
    /// This is probably not what you want.
    pub async fn next(
        &mut self,
    ) -> Option<(
        AddressedHolochainP2pMockMsg,
        Option<HolochainP2pMockRespond>,
    )> {
        match self.from_kitsune.next().await {
            Some(msg) => {
                let to_agent = self
                    .address_map
                    .iter()
                    .find_map(|(agent, (cert, _))| {
                        if cert == msg.cert() {
                            Some(agent.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap();

                let (msg, respond) = msg.into_msg_respond();

                let msg = HolochainP2pMockMsg::from_wire_msg(msg);

                let respond = respond.map(|respond| HolochainP2pMockRespond { respond });
                Some((msg.addressed(to_agent), respond))
            }
            None => None,
        }
    }

    /// Send a notify or request from an addressed simulated agent.
    /// If this is a request you will get back the response.
    pub async fn send(&self, msg: AddressedHolochainP2pMockMsg) -> Option<HolochainP2pMockMsg> {
        let AddressedHolochainP2pMockMsg { msg, agent: from } = msg;
        let (cert, url) = self.address_map.get(&from).cloned().unwrap();
        let id = msg.to_id();
        let (msg, rx) = if id.is_notify() {
            (
                KitsuneMock::notify(id, cert, url, msg.into_wire_msg()),
                None,
            )
        } else {
            let (respond, rx) = tokio::sync::oneshot::channel();

            (
                KitsuneMock::request(id, cert, url, msg.into_wire_msg(), respond),
                Some(rx),
            )
        };
        let _ = self.to_kitsune.send(msg).await;
        match rx {
            Some(rx) => rx
                .await
                .ok()
                .map(|k| HolochainP2pMockMsg::from_wire_msg(k.into_wire())),
            None => None,
        }
    }
}

impl HolochainP2pMockMsg {
    /// Associate a message with the simulated agent that is sending or receiving
    /// this message. From holochain's point of view this is the remote node that
    /// the message is being sent to or received from.
    pub fn addressed(self, agent: AgentPubKey) -> AddressedHolochainP2pMockMsg {
        AddressedHolochainP2pMockMsg { msg: self, agent }
    }

    /// Generate the correct message id associated with this message.
    fn to_id(&self) -> MsgId {
        match self {
            HolochainP2pMockMsg::Wire { msg, .. } => match &msg {
                crate::wire::WireMessage::CallRemote { .. }
                | crate::wire::WireMessage::ValidationReceipt { .. }
                | crate::wire::WireMessage::Get { .. }
                | crate::wire::WireMessage::GetMeta { .. }
                | crate::wire::WireMessage::GetLinks { .. }
                | crate::wire::WireMessage::GetAgentActivity { .. }
                | crate::wire::WireMessage::MustGetAgentActivity { .. } => next_msg_id().as_req(),

                crate::wire::WireMessage::Publish { .. }
                | crate::wire::WireMessage::CountersigningSessionNegotiation { .. } => {
                    MsgId::new_notify()
                }
            },
            HolochainP2pMockMsg::PeerGet(_) | HolochainP2pMockMsg::PeerQuery(_) => {
                next_msg_id().as_req()
            }
            HolochainP2pMockMsg::Gossip { .. } => MsgId::new_notify(),
            _ => panic!("Should not be sending responses"),
        }
    }

    /// Turn a mock message into a kitsune wire message.
    fn into_wire_msg(self) -> kwire::Wire {
        match self {
            HolochainP2pMockMsg::Wire {
                to_agent, msg, dna, ..
            } => {
                let call = match &msg {
                    crate::wire::WireMessage::CallRemote { .. }
                    | crate::wire::WireMessage::ValidationReceipt { .. }
                    | crate::wire::WireMessage::Get { .. }
                    | crate::wire::WireMessage::GetMeta { .. }
                    | crate::wire::WireMessage::GetLinks { .. }
                    | crate::wire::WireMessage::GetAgentActivity { .. }
                    | crate::wire::WireMessage::MustGetAgentActivity { .. } => true,

                    crate::wire::WireMessage::Publish { .. }
                    | crate::wire::WireMessage::CountersigningSessionNegotiation { .. } => false,
                };
                let to_agent = to_agent.to_kitsune();
                let space = dna.to_kitsune();
                let data = msg.encode().unwrap().into();
                if call {
                    kwire::Wire::Call(kwire::Call {
                        space,
                        to_agent,
                        data,
                    })
                } else {
                    kwire::Wire::Broadcast(kwire::Broadcast {
                        space,
                        to_agent,
                        data,
                        destination: BroadcastTo::Notify,
                    })
                }
            }
            HolochainP2pMockMsg::CallResp(data) => kwire::Wire::call_resp(data),
            HolochainP2pMockMsg::PeerGet(data) => kwire::Wire::PeerGet(data),
            HolochainP2pMockMsg::PeerGetResp(data) => kwire::Wire::PeerGetResp(data),
            HolochainP2pMockMsg::PeerQuery(data) => kwire::Wire::PeerQuery(data),
            HolochainP2pMockMsg::PeerQueryResp(data) => kwire::Wire::PeerQueryResp(data),
            HolochainP2pMockMsg::Gossip {
                dna,
                module,
                gossip,
            } => {
                use kitsune_p2p_types::codec::Codec;
                let space = dna.to_kitsune();
                let data = match gossip {
                    GossipProtocol::Simple => todo!(),
                    GossipProtocol::Sharded(gossip) => gossip.encode_vec().unwrap().into(),
                };
                kwire::Wire::Gossip(kwire::Gossip {
                    space,
                    data,
                    module,
                })
            }
            HolochainP2pMockMsg::MetricExchange(msg) => kwire::Wire::MetricExchange(msg),
            HolochainP2pMockMsg::Failure(reason) => kwire::Wire::Failure(kwire::Failure { reason }),
            HolochainP2pMockMsg::PublishedAgentInfo {
                to_agent,
                dna,
                info,
            } => {
                let space = dna.to_kitsune();
                let to_agent = to_agent.to_kitsune();
                let data = info.encode().unwrap().to_vec().into();
                kwire::Wire::Broadcast(kwire::Broadcast {
                    space,
                    to_agent,
                    data,
                    destination: BroadcastTo::PublishAgentInfo,
                })
            }
        }
    }

    fn from_wire_msg(msg: kwire::Wire) -> Self {
        match msg {
            kwire::Wire::Call(kwire::Call {
                to_agent,
                data,
                space,
            }) => {
                let to_agent = holo_hash::AgentPubKey::from_kitsune(&to_agent);
                let dna = holo_hash::DnaHash::from_kitsune(&space);
                let msg = crate::wire::WireMessage::decode(data.as_ref()).unwrap();
                HolochainP2pMockMsg::Wire { to_agent, msg, dna }
            }
            kwire::Wire::Broadcast(kwire::Broadcast {
                to_agent,
                data,
                space,
                destination,
                ..
            })
            | kwire::Wire::DelegateBroadcast(kwire::DelegateBroadcast {
                to_agent,
                data,
                space,
                destination,
                ..
            }) => {
                let to_agent = holo_hash::AgentPubKey::from_kitsune(&to_agent);
                let dna = holo_hash::DnaHash::from_kitsune(&space);
                match destination {
                    BroadcastTo::Notify => {
                        let msg = crate::wire::WireMessage::decode(data.as_ref()).unwrap();
                        HolochainP2pMockMsg::Wire { to_agent, msg, dna }
                    }
                    BroadcastTo::PublishAgentInfo => {
                        let info = AgentInfoSigned::decode(&data[..]).unwrap();
                        HolochainP2pMockMsg::PublishedAgentInfo {
                            to_agent,
                            dna,
                            info,
                        }
                    }
                }
            }
            kwire::Wire::Gossip(kwire::Gossip {
                data,
                space,
                module,
            }) => {
                use kitsune_p2p::gossip::sharded_gossip::*;
                use kitsune_p2p_types::codec::Codec;
                let gossip = match module {
                    GossipModuleType::Simple => todo!(),
                    GossipModuleType::ShardedRecent | GossipModuleType::ShardedHistorical => {
                        GossipProtocol::Sharded(
                            ShardedGossipWire::decode_ref(data.as_ref()).unwrap().1,
                        )
                    }
                };
                let dna = holo_hash::DnaHash::from_kitsune(&space);
                HolochainP2pMockMsg::Gossip {
                    module,
                    dna,
                    gossip,
                }
            }
            kwire::Wire::MetricExchange(msg) => HolochainP2pMockMsg::MetricExchange(msg),
            kwire::Wire::PeerGet(msg) => HolochainP2pMockMsg::PeerGet(msg),
            kwire::Wire::PeerGetResp(msg) => HolochainP2pMockMsg::PeerGetResp(msg),
            kwire::Wire::PeerQuery(msg) => HolochainP2pMockMsg::PeerQuery(msg),
            kwire::Wire::PeerQueryResp(msg) => HolochainP2pMockMsg::PeerQueryResp(msg),
            kwire::Wire::CallResp(msg) => HolochainP2pMockMsg::CallResp(msg.data),
            kwire::Wire::Failure(msg) => HolochainP2pMockMsg::Failure(msg.reason),
        }
    }
}
