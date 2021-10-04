//! Utilities for mocking the holochain network.

#![allow(missing_docs)]

use fixt::prelude::Distribution;
use futures::stream::Stream;
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
use observability::tracing;

static MSG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_msg_id() -> MsgId {
    MsgId::new(MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

pub struct HolochainP2pMockChannel {
    address_map: HashMap<AgentPubKey, (Tx2Cert, TxUrl)>,
    from_kitsune: Pin<Box<dyn Stream<Item = KitsuneMock> + Send + Sync + 'static>>,
    to_kitsune: ToKitsuneMockChannelTx,
}

#[derive(Clone)]
pub struct MockScenario {
    /// Percentage of messages that will be dropped.
    pub percent_drop_msg: f32,
    /// Percentage of nodes that will not respond.
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
pub struct AddressedHolochainP2pMockMsg {
    pub msg: HolochainP2pMockMsg,
    pub agent: AgentPubKey,
}

#[derive(Debug)]
pub enum HolochainP2pMockMsg {
    Wire {
        to_agent: AgentPubKey,
        from_agent: Option<AgentPubKey>,
        dna: DnaHash,
        msg: super::wire::WireMessage,
    },
    CallResp(kitsune_p2p::wire::WireData),
    PeerGet(kitsune_p2p::wire::PeerGet),
    PeerGetResp(kitsune_p2p::wire::PeerGetResp),
    PeerQuery(kitsune_p2p::wire::PeerQuery),
    PeerQueryResp(kitsune_p2p::wire::PeerQueryResp),
    Gossip {
        dna: DnaHash,
        module: GossipModuleType,
        gossip: kitsune_p2p::gossip::sharded_gossip::ShardedGossipWire,
    },
}

impl HolochainP2pMockMsg {
    pub fn addressed(self, from: AgentPubKey) -> AddressedHolochainP2pMockMsg {
        AddressedHolochainP2pMockMsg {
            msg: self,
            agent: from,
        }
    }
}

pub struct HolochainP2pMockRespond {
    respond: KitsuneMockRespond,
}

fn to_id(msg: &HolochainP2pMockMsg) -> MsgId {
    match msg {
        HolochainP2pMockMsg::Wire { msg, .. } => match &msg {
            crate::wire::WireMessage::CallRemote { .. }
            | crate::wire::WireMessage::ValidationReceipt { .. }
            | crate::wire::WireMessage::Get { .. }
            | crate::wire::WireMessage::GetMeta { .. }
            | crate::wire::WireMessage::GetLinks { .. }
            | crate::wire::WireMessage::GetAgentActivity { .. }
            | crate::wire::WireMessage::GetValidationPackage { .. } => next_msg_id().as_req(),
            crate::wire::WireMessage::Publish { .. }
            | crate::wire::WireMessage::CountersigningAuthorityResponse { .. } => {
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
fn to_wire_msg(msg: HolochainP2pMockMsg) -> kwire::Wire {
    match msg {
        HolochainP2pMockMsg::Wire {
            to_agent,
            msg,
            from_agent,
            dna,
        } => {
            let call = match &msg {
                crate::wire::WireMessage::CallRemote { .. }
                | crate::wire::WireMessage::ValidationReceipt { .. }
                | crate::wire::WireMessage::Get { .. }
                | crate::wire::WireMessage::GetMeta { .. }
                | crate::wire::WireMessage::GetLinks { .. }
                | crate::wire::WireMessage::GetAgentActivity { .. }
                | crate::wire::WireMessage::GetValidationPackage { .. } => true,
                crate::wire::WireMessage::Publish { .. }
                | crate::wire::WireMessage::CountersigningAuthorityResponse { .. } => false,
            };
            let to_agent = to_agent.to_kitsune();
            let space = dna.to_kitsune();
            let data = msg.encode().unwrap().into();
            if call {
                let from_agent = from_agent.unwrap().to_kitsune();
                kwire::Wire::Call(kwire::Call {
                    to_agent,
                    space,
                    from_agent,
                    data,
                })
            } else {
                kwire::Wire::Broadcast(kwire::Broadcast {
                    to_agent,
                    space,
                    data,
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
            let data = gossip.encode_vec().unwrap().into();
            kwire::Wire::Gossip(kwire::Gossip {
                space,
                module,
                data,
            })
        }
    }
}
impl HolochainP2pMockRespond {
    pub fn respond(self, msg: HolochainP2pMockMsg) {
        self.respond.respond(to_wire_msg(msg));
    }
}

impl HolochainP2pMockChannel {
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
                let url = info.url_list.iter().next().cloned().unwrap();
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
            let scenario = scenario.clone();
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

    pub async fn next(
        &mut self,
    ) -> Option<(
        AddressedHolochainP2pMockMsg,
        Option<HolochainP2pMockRespond>,
    )> {
        while let Some(msg) = self.from_kitsune.next().await {
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

            let needs_response;
            let msg = match msg {
                kwire::Wire::Call(kwire::Call {
                    to_agent,
                    data,
                    space,
                    from_agent,
                }) => {
                    let to_agent = holo_hash::AgentPubKey::from_kitsune(&to_agent);
                    let from_agent = holo_hash::AgentPubKey::from_kitsune(&from_agent);
                    let dna = holo_hash::DnaHash::from_kitsune(&space);
                    let msg = crate::wire::WireMessage::decode(data.as_ref()).unwrap();
                    needs_response = true;
                    HolochainP2pMockMsg::Wire {
                        to_agent,
                        msg,
                        dna,
                        from_agent: Some(from_agent),
                    }
                }
                kwire::Wire::Broadcast(kwire::Broadcast {
                    to_agent,
                    data,
                    space,
                    ..
                })
                | kwire::Wire::DelegateBroadcast(kwire::DelegateBroadcast {
                    to_agent,
                    data,
                    space,
                    ..
                }) => {
                    let to_agent = holo_hash::AgentPubKey::from_kitsune(&to_agent);
                    let dna = holo_hash::DnaHash::from_kitsune(&space);
                    let msg = crate::wire::WireMessage::decode(data.as_ref()).unwrap();
                    needs_response = false;
                    HolochainP2pMockMsg::Wire {
                        to_agent,
                        msg,
                        dna,
                        from_agent: None,
                    }
                }
                kwire::Wire::Gossip(kwire::Gossip {
                    data,
                    space,
                    module,
                }) => {
                    use kitsune_p2p::gossip::sharded_gossip::*;
                    use kitsune_p2p_types::codec::Codec;
                    let (_, gossip) = ShardedGossipWire::decode_ref(data.as_ref()).unwrap();
                    let dna = holo_hash::DnaHash::from_kitsune(&space);
                    needs_response = false;
                    HolochainP2pMockMsg::Gossip {
                        module,
                        dna,
                        gossip,
                    }
                }
                kwire::Wire::PeerGet(msg) => {
                    needs_response = true;
                    HolochainP2pMockMsg::PeerGet(msg)
                }
                kwire::Wire::PeerGetResp(msg) => {
                    needs_response = false;
                    HolochainP2pMockMsg::PeerGetResp(msg)
                }
                kwire::Wire::PeerQuery(msg) => {
                    needs_response = true;
                    HolochainP2pMockMsg::PeerQuery(msg)
                }
                kwire::Wire::PeerQueryResp(msg) => {
                    needs_response = false;
                    HolochainP2pMockMsg::PeerQueryResp(msg)
                }
                kwire::Wire::CallResp(msg) => {
                    needs_response = false;
                    HolochainP2pMockMsg::CallResp(msg.data)
                }
                kwire::Wire::Failure(msg) => {
                    tracing::error!("HolochainP2pMock Failure {}", msg.reason);
                    continue;
                }
            };

            let respond = if needs_response {
                match respond {
                    Some(respond) => Some(HolochainP2pMockRespond { respond }),
                    None => None,
                }
            } else {
                None
            };
            return Some((msg.addressed(to_agent), respond));
        }
        None
    }

    pub async fn send(&self, msg: AddressedHolochainP2pMockMsg) -> Option<HolochainP2pMockMsg> {
        let AddressedHolochainP2pMockMsg { msg, agent: from } = msg;
        let (cert, url) = self.address_map.get(&from).cloned().unwrap();
        let id = to_id(&msg);
        let msg = if id.is_notify() {
            KitsuneMock::notify(id, cert, url, to_wire_msg(msg))
        } else {
            let (respond, _) = tokio::sync::oneshot::channel();

            KitsuneMock::request(id, cert, url, to_wire_msg(msg), respond)
        };
        let _ = self.to_kitsune.send(msg).await;
        if id.is_req() {
            todo!("Add the ability to send requests to holochain")
        } else {
            None
        }
    }
}
