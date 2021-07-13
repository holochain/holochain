use crate::agent_store::AgentInfoSigned;
use crate::gossip::simple_bloom::{decode_bloom_filter, encode_bloom_filter};
use crate::types::event::*;
use crate::types::gossip::*;
use crate::types::*;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArcSet};
use kitsune_p2p_types::metrics::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::simple_bloom::{HowToConnect, MetaOpKey};

mod accept;
mod agents;
mod bloom;
mod initiate;
mod ops;
mod store;

/// max send buffer size (keep it under 16384 with a little room for overhead)
/// (this is not a tuning_param because it must be coordinated
/// with the constant in PoolBuf which cannot be set at runtime)
const MAX_SEND_BUF_BYTES: usize = 16000;

type BloomFilter = bloomfilter::Bloom<Arc<MetaOpKey>>;
type EventSender = futures::channel::mpsc::Sender<event::KitsuneP2pEvent>;

#[derive(Debug, Clone, Copy)]
enum GossipType {
    Recent,
    Historical,
}

struct ShardedGossip {
    gossip_type: GossipType,
    tuning_params: KitsuneP2pTuningParams,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    space: Arc<KitsuneSpace>,
    evt_sender: EventSender,
    inner: Share<ShardedGossipInner>,
}

/// Incoming gossip.
type Incoming = (Tx2ConHnd<wire::Wire>, ShardedGossipWire);
/// Outgoing gossip.
type Outgoing = (GossipTgt, HowToConnect, ShardedGossipWire);

type StateKey = Tx2Cert;

struct ShardedGossipInner {
    local_agents: HashSet<Arc<KitsuneAgent>>,
    initiate_tgt: Option<GossipTgt>,
    incoming: VecDeque<Incoming>,
    outgoing: VecDeque<Outgoing>,
    // TODO: Figure out how to properly clean up old
    // gossip round states.
    state_map: HashMap<StateKey, RoundState>,
}

#[derive(Debug, Clone)]
struct RoundState {
    common_arc_set: Arc<DhtArcSet>,
    since_ms: u64,
    until_ms: u64,
}

impl ShardedGossip {
    const TGT_FP: f64 = 0.01;

    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
        gossip_type: GossipType,
    ) -> Arc<Self> {
        let this = Arc::new(Self {
            tuning_params,
            space,
            ep_hnd,
            // send_interval_ms,
            evt_sender,
            inner: Share::new(ShardedGossipInner::new()),
            gossip_type,
        });
        metric_task({
            let this = this.clone();
            async move {
                loop {
                    // TODO: Use parameters for sleep time
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    this.run_one_iteration().await;
                }
                KitsuneResult::Ok(())
            }
        });
        this
    }

    async fn run_one_iteration(&self) -> () {
        // TODO: Handle errors
        self.try_initiate().await.unwrap();
        self.process_incoming_outgoing().await.unwrap();
    }

    /// Calculate the time range for a gossip round.
    fn calculate_time_range(&self) -> KitsuneResult<(u64, u64)> {
        // TODO: This is where we need to actually choose
        // an appropriate gossip window based on the type of
        // gossip (recent vs historical) and maybe the amount
        // of ops?

        // Blooms optimize for lots of new data.
        // Hashes optimize no recent changes.
        Ok((0, u64::MAX))
    }

    async fn process_incoming_outgoing(&self) -> KitsuneResult<()> {
        let (incoming, outgoing) = self.pop_queues()?;
        if let Some(incoming) = incoming {
            self.process_incoming(incoming).await?;
        }
        if let Some(outgoing) = outgoing {
            self.process_outgoing(outgoing).await?;
        }
        // TODO: Locally sync agents.
        Ok(())
    }

    fn pop_queues(&self) -> KitsuneResult<(Option<Incoming>, Option<Outgoing>)> {
        self.inner.share_mut(move |inner, _| {
            let incoming = inner.incoming.pop_front();
            let outgoing = inner.outgoing.pop_front();
            Ok((incoming, outgoing))
        })
    }

    fn new_state(&self, common_arc_set: Arc<DhtArcSet>) -> KitsuneResult<RoundState> {
        let (since_ms, until_ms) = self.calculate_time_range()?;
        Ok(RoundState {
            common_arc_set,
            since_ms,
            until_ms,
        })
    }

    async fn get_state(&self, id: StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner
            .share_mut(|i, _| Ok(i.state_map.get(&id).cloned()))
    }

    async fn remove_state(&self, id: StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| Ok(i.state_map.remove(&id)))
    }

    async fn process_incoming(&self, incoming: Incoming) -> KitsuneResult<()> {
        // TODO: How do we route the gossip to the right loop type (recent vs historical)
        let (con, gossip) = incoming;
        match gossip {
            ShardedGossipWire::Initiate(Initiate { intervals }) => {
                self.incoming_initiate(con, intervals).await?
            }
            ShardedGossipWire::Accept(Accept { intervals }) => {
                self.incoming_accept(con, intervals).await?
            }
            ShardedGossipWire::Agents(Agents { filter }) => {
                if let Some(state) = self.get_state(con.peer_cert()).await? {
                    let filter = decode_bloom_filter(&filter);
                    self.incoming_agents(con, state, filter).await?;
                }
            }
            ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                if let Some(state) = self.get_state(con.peer_cert()).await? {
                    self.incoming_missing_agents(state, agents).await?;
                }
            }
            ShardedGossipWire::Ops(Ops { filter }) => {
                if let Some(state) = self.get_state(con.peer_cert()).await? {
                    let filter = decode_bloom_filter(&filter);
                    self.incoming_ops(con, state, filter).await?
                }
            }
            ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                let state = if finished {
                    self.remove_state(con.peer_cert()).await?
                } else {
                    self.get_state(con.peer_cert()).await?
                };
                if let Some(state) = state {
                    self.incoming_missing_ops(state, ops).await?;
                }
            }

            _ => todo!(),
        }
        Ok(())
    }

    async fn process_outgoing(&self, outgoing: Outgoing) -> KitsuneResult<()> {
        let (endpoint, how, gossip) = outgoing;
        let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
        let sending_bytes = gossip.len();
        let gossip = wire::Wire::gossip(self.space.clone(), gossip.into());

        let timeout = self.tuning_params.implicit_timeout();

        let con = match how {
            HowToConnect::Con(con) => {
                // TODO: Uncomment this and make it work.
                // if con.is_closed() {
                //     let url = pick_url_for_cert(inner, &peer_cert)?;
                //     ep_hnd.get_connection(url, t).await?
                // } else {
                //     con
                // }
                con
            }
            HowToConnect::Url(url) => self.ep_hnd.get_connection(url, timeout).await?,
        };
        // TODO: Wait for bandwidth enough available outgoing bandwidth here before
        // actually sending the gossip.
        con.notify(&gossip, timeout).await?;
        Ok(())
    }

    /// Find a remote endpoint from agents within arc set.
    async fn find_remote_agent_within_arc(
        &self,
        agent: &Arc<KitsuneAgent>,
        arc_set: Arc<DhtArcSet>,
        local_agents: &HashSet<Arc<KitsuneAgent>>,
    ) -> KitsuneResult<Option<(GossipTgt, TxUrl)>> {
        // Get the time range for this gossip.
        let (since_ms, until_ms) = self.calculate_time_range()?;
        let mut remote_agents_within_arc_set = store::agents_within_arcset(
            &self.evt_sender,
            &self.space,
            &agent,
            arc_set.clone(),
            since_ms,
            until_ms,
        )
        .await?
        .into_iter()
        .filter(|(a, _)| !local_agents.contains(a));

        // Get the first remote endpoint.
        // TODO: Make this more intelligent and don't just choose the first.
        match remote_agents_within_arc_set.next() {
            Some((remote_agent, _)) => {
                Ok(
                    // Get the agent info for the chosen remote agent.
                    store::get_agent_info(&self.evt_sender, &self.space, &remote_agent)
                        .await
                        // TODO: Handle error.
                        .unwrap()
                        .and_then(|ra| {
                            ra.url_list.get(0).cloned().and_then(|url| {
                                kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str())
                                    .map_err(|e| tracing::error!("Failed to parse url {:?}", e))
                                    .ok()
                                    .map(|purl| {
                                        (
                                            GossipTgt::new(
                                                vec![ra.agent.clone()],
                                                Tx2Cert::from(purl.digest()),
                                            ),
                                            TxUrl::from(url.as_str()),
                                        )
                                    })
                            })
                        }),
                )
            }
            None => Ok(None),
        }
    }
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec ShardedGossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            intervals.0: Vec<ArcInterval>,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            intervals.0: Vec<ArcInterval>,
        },

        /// Send Agent Info Boom
        Agents(0x30) {
            filter.0: PoolBuf,
        },

        /// Any agents that were missing from the bloom.
        MissingAgents(0x40) {
            agents.0: Vec<Arc<AgentInfoSigned>>,
        },

        /// Send Agent Info Boom
        Ops(0x50) {
            filter.0: PoolBuf,
        },

        /// Any ops that were missing from the remote bloom.
        MissingOps(0x60) {
            ops.0: Vec<Arc<(Arc<KitsuneOpHash>, Vec<u8>)>>,
            finished.1: bool,
        },
    }
}

impl ShardedGossipInner {
    fn new() -> Self {
        Self {
            local_agents: Default::default(),
            initiate_tgt: Default::default(),
            incoming: Default::default(),
            outgoing: Default::default(),
            state_map: Default::default(),
        }
    }
}

impl AsGossipModule for ShardedGossip {
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        use kitsune_p2p_types::codec::*;
        let (_, gossip) =
            ShardedGossipWire::decode_ref(&gossip_data).map_err(KitsuneError::other)?;
        self.inner.share_mut(move |i, _| {
            i.incoming.push_back((con, gossip));
            if i.incoming.len() > 20 {
                tracing::warn!(
                    "Overloaded with incoming gossip.. {} messages",
                    i.incoming.len()
                );
            }
            Ok(())
        })
    }

    fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.inner.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }
}

struct ShardedGossipFactory;

impl AsGossipModuleFactory for ShardedGossipFactory {
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> GossipModule {
        GossipModule(ShardedGossip::new(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            GossipType::Recent,
        ))
    }
}
pub fn factory() -> GossipModuleFactory {
    GossipModuleFactory(Arc::new(ShardedGossipFactory))
}

fn clamp64(u: u64) -> i64 {
    if u > i64::MAX as u64 {
        i64::MAX
    } else {
        u as i64
    }
}
