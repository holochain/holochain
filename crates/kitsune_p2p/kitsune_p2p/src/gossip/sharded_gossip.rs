//! The main (and only) Sharded gossiping strategy

#![warn(missing_docs)]

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
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use self::initiate::decode_timed_bloom_filter;
use self::state_map::RoundStateMap;

use super::simple_bloom::{HowToConnect, MetaOpKey};

mod accept;
mod agents;
mod bloom;
mod initiate;
mod ops;
mod state_map;
mod store;

#[cfg(test)]
mod tests;

/// max send buffer size (keep it under 16384 with a little room for overhead)
/// (this is not a tuning_param because it must be coordinated
/// with the constant in PoolBuf which cannot be set at runtime)
const MAX_SEND_BUF_BYTES: usize = 16000;

type BloomFilter = bloomfilter::Bloom<Arc<MetaOpKey>>;
type EventSender = futures::channel::mpsc::Sender<event::KitsuneP2pEvent>;

struct TimedBloomFilter {
    bloom: BloomFilter,
    time: std::ops::Range<u64>,
}

/// Gossip has two distinct variants which share a lot of similarities but
/// are fundamentally different and serve different purposes
#[derive(Debug, Clone, Copy)]
pub enum GossipType {
    /// The Recent gossip type is aimed at rapidly syncing the most recent
    /// data. It runs frequently and expects frequent diffs at each round.
    Recent,
    /// The Historical gossip type is aimed at comprehensively syncing the
    /// entire common history of two nodes, filling in gaps in the historical
    /// data. It runs less frequently, and expects diffs to be infrequent
    /// at each round.
    #[allow(dead_code)]
    Historical,
}

/// The entry point for the sharded gossip strategy.
///
/// This struct encapsulates the network communication concerns, mainly
/// managing the incoming and outgoing gossip queues. It contains a struct
/// which handles all other (local) aspects of gossip.
pub struct ShardedGossip {
    /// ShardedGossipLocal handles the non-networking concerns of gossip
    gossip: ShardedGossipLocal,
    // The endpoint to use for all outgoing comms
    ep_hnd: Tx2EpHnd<wire::Wire>,
    /// The internal mutable state
    inner: Share<ShardedGossipState>,
}

impl ShardedGossip {
    /// Constructor
    pub fn new(
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: EventSender,
        gossip_type: GossipType,
    ) -> Arc<Self> {
        let mut inner = ShardedGossipLocalState::default();
        inner.tuning_params = tuning_params.clone();
        let this = Arc::new(Self {
            ep_hnd,
            inner: Share::new(Default::default()),
            gossip: ShardedGossipLocal {
                tuning_params,
                space,
                evt_sender,
                inner: Share::new(inner),
                gossip_type,
            },
        });
        metric_task({
            let this = this.clone();

            #[allow(unreachable_code)]
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

    async fn process_outgoing(&self, outgoing: Outgoing) -> KitsuneResult<()> {
        let (_endpoint, how, gossip) = outgoing;
        let gossip = gossip.encode_vec().map_err(KitsuneError::other)?;
        let gossip = wire::Wire::gossip(self.gossip.space.clone(), gossip.into());

        let timeout = self.gossip.tuning_params.implicit_timeout();

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
        // TODO: Wait for enough available outgoing bandwidth here before
        // actually sending the gossip.
        con.notify(&gossip, timeout).await?;
        Ok(())
    }

    async fn process_incoming_outgoing(&self) -> KitsuneResult<()> {
        let (incoming, outgoing) = self.pop_queues()?;
        if let Some((con, msg)) = incoming {
            let outgoing = self.gossip.process_incoming(con.peer_cert(), msg).await?;
            self.inner.share_mut(|i, _| {
                i.outgoing.extend(outgoing.into_iter().map(|msg| {
                    (
                        GossipTgt::new(Vec::with_capacity(0), con.peer_cert()),
                        HowToConnect::Con(con.clone()),
                        msg,
                    )
                }));
                Ok(())
            })?;
        }
        if let Some(outgoing) = outgoing {
            self.process_outgoing(outgoing).await?;
        }
        // TODO: Locally sync agents.
        Ok(())
    }

    async fn run_one_iteration(&self) -> () {
        // TODO: Handle errors
        if let Some(outgoing) = self.gossip.try_initiate().await.unwrap() {
            self.inner
                .share_mut(|i, _| {
                    i.outgoing.push_back(outgoing);
                    Ok(())
                })
                .unwrap();
        }
        self.process_incoming_outgoing().await.unwrap();
    }

    fn pop_queues(&self) -> KitsuneResult<(Option<Incoming>, Option<Outgoing>)> {
        self.inner.share_mut(move |inner, _| {
            let incoming = inner.incoming.pop_front();
            let outgoing = inner.outgoing.pop_front();
            Ok((incoming, outgoing))
        })
    }
}

/// The parts of sharded gossip which are concerned only with the gossiping node:
/// - managing local state
/// - making requests to the local backend
/// - processing incoming messages to produce outgoing messages (which actually)
///     get sent by the enclosing `ShardedGossip`
pub struct ShardedGossipLocal {
    gossip_type: GossipType,
    tuning_params: KitsuneP2pTuningParams,
    space: Arc<KitsuneSpace>,
    evt_sender: EventSender,
    inner: Share<ShardedGossipLocalState>,
}

/// Incoming gossip.
type Incoming = (Tx2ConHnd<wire::Wire>, ShardedGossipWire);
/// Outgoing gossip.
type Outgoing = (GossipTgt, HowToConnect, ShardedGossipWire);

type StateKey = Tx2Cert;

/// The internal mutable state for [`ShardedGossipLocal`]
#[derive(Default)]
pub struct ShardedGossipLocalState {
    /// The list of agents on this node
    local_agents: HashSet<Arc<KitsuneAgent>>,
    tuning_params: KitsuneP2pTuningParams,
    /// If Some, we are in the process of trying to initiate gossip with this target.
    initiate_tgt: Option<(GossipTgt, u8)>,
    // TODO: Figure out how to properly clean up old
    // gossip round states.
    round_map: RoundStateMap,
}

impl ShardedGossipLocalState {
    fn remove_state(&mut self, state_key: &StateKey) -> Option<RoundState> {
        if self
            .initiate_tgt
            .as_ref()
            .map(|tgt| tgt.0.cert() == state_key)
            .unwrap_or(false)
        {
            self.initiate_tgt = None;
        }
        self.round_map.remove(state_key)
    }

    fn check_tgt_expired(&mut self) {
        if let Some(cert) = self.initiate_tgt.as_ref().map(|tgt| tgt.0.cert().clone()) {
            if self.round_map.check_timeout(&cert) {
                self.initiate_tgt = None;
            }
        }
    }
}

/// The internal mutable state for [`ShardedGossip`]
#[derive(Default)]
pub struct ShardedGossipState {
    incoming: VecDeque<Incoming>,
    outgoing: VecDeque<Outgoing>,
}

/// The state representing a single active ongoing "round" of gossip with a
/// remote node
#[derive(Debug, Clone)]
pub struct RoundState {
    /// The common ground with our gossip partner for the purposes of this round
    common_arc_set: Arc<DhtArcSet>,
    /// Number of ops blooms we have sent for this round, which is also the
    /// number of MissingOps sets we expect in response
    num_ops_blooms: u8,
    /// We've received the last op bloom filter from our partner
    /// (the one with `finished` == true)
    increment_ops_complete: bool,
    /// Round start time
    created_at: std::time::Instant,
    /// Amount of time before a round is considered expired.
    round_timeout: u32,
}

impl ShardedGossipLocal {
    const TGT_FP: f64 = 0.01;
    /// This should give us just under 16MB for the bloom filter.
    /// Based on a compression of 75%.
    const UPPER_HASHES_BOUND: usize = 500;

    /// Calculate the time range for a gossip round.
    fn calculate_time_ranges(&self) -> Vec<Range<u64>> {
        const NOW: Duration = Duration::from_secs(0);
        const HOUR: Duration = Duration::from_secs(60 * 60);
        const DAY: Duration = Duration::from_secs(60 * 60 * 24);
        const WEEK: Duration = Duration::from_secs(60 * 60 * 24 * 7);
        const MONTH: Duration = Duration::from_secs(60 * 60 * 24 * 7 * 30);
        const START_OF_TIME: Duration = Duration::MAX;
        match self.gossip_type {
            GossipType::Recent => {
                vec![time_range(HOUR, NOW)]
            }
            GossipType::Historical => {
                vec![
                    time_range(DAY, HOUR),
                    time_range(WEEK, DAY),
                    time_range(MONTH, WEEK),
                    time_range(START_OF_TIME, MONTH),
                ]
            }
        }
    }

    fn new_state(&self, common_arc_set: Arc<DhtArcSet>) -> KitsuneResult<RoundState> {
        Ok(RoundState {
            common_arc_set,
            num_ops_blooms: 0,
            increment_ops_complete: false,
            created_at: std::time::Instant::now(),
            // TODO: Check if the node is a successful peer or not and set the timeout accordingly
            round_timeout: self
                .tuning_params
                .gossip_peer_on_success_next_gossip_delay_ms,
        })
    }

    async fn get_state(&self, id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner
            .share_mut(|i, _| Ok(i.round_map.get(id).cloned()))
    }

    async fn remove_state(&self, id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| Ok(i.remove_state(id)))
    }

    async fn incoming_ops_finished(
        &self,
        state_id: &StateKey,
    ) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            if i.round_map
                .get_mut(state_id)
                .map(|state| {
                    state.increment_ops_complete = true;
                    state.num_ops_blooms == 0
                })
                .unwrap_or(true)
            {
                Ok(i.remove_state(state_id))
            } else {
                Ok(i.round_map.get(state_id).cloned())
            }
        })
    }

    async fn decrement_ops_blooms(&self, state_id: &StateKey) -> KitsuneResult<Option<RoundState>> {
        self.inner.share_mut(|i, _| {
            if i.round_map
                .get_mut(state_id)
                .map(|state| {
                    let num_ops_blooms = state.num_ops_blooms.checked_sub(1).unwrap_or(0);
                    state.num_ops_blooms = num_ops_blooms;
                    state.num_ops_blooms == 0 && state.increment_ops_complete
                })
                .unwrap_or(true)
            {
                Ok(i.remove_state(state_id))
            } else {
                Ok(i.round_map.get(state_id).cloned())
            }
        })
    }

    async fn process_incoming(
        &self,
        cert: Tx2Cert,
        msg: ShardedGossipWire,
    ) -> KitsuneResult<Vec<ShardedGossipWire>> {
        // TODO: How do we route the gossip to the right loop type (recent vs historical)

        // If we don't have the state for a message then the other node will need to timeout.
        Ok(match msg {
            ShardedGossipWire::Initiate(Initiate { intervals, id }) => {
                self.incoming_initiate(cert, intervals, id).await?
            }
            ShardedGossipWire::Accept(Accept { intervals }) => {
                self.incoming_accept(cert, intervals).await?
            }
            ShardedGossipWire::Agents(Agents { filter }) => {
                if let Some(state) = self.get_state(&cert).await? {
                    let filter = decode_bloom_filter(&filter);
                    self.incoming_agents(state, filter).await?
                } else {
                    Vec::with_capacity(0)
                }
            }
            ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                if let Some(state) = self.get_state(&cert).await? {
                    self.incoming_missing_agents(state, agents).await?;
                }
                Vec::with_capacity(0)
            }
            ShardedGossipWire::Ops(Ops { filter, finished }) => {
                let state = if finished {
                    self.incoming_ops_finished(&cert).await?
                } else {
                    self.get_state(&cert).await?
                };
                match (state, filter) {
                    (Some(state), Some(filter)) => {
                        let filter = decode_timed_bloom_filter(&filter);
                        self.incoming_ops(state.clone(), filter).await?
                    }
                    _ => Vec::with_capacity(0),
                }
            }
            ShardedGossipWire::MissingOps(MissingOps { ops, finished }) => {
                let state = if finished {
                    self.decrement_ops_blooms(&cert).await?
                } else {
                    self.get_state(&cert).await?
                };
                if let Some(state) = state {
                    self.incoming_missing_ops(state, ops).await?;
                }
                Vec::with_capacity(0)
            }
            ShardedGossipWire::NoAgents(_) => {
                self.remove_state(&cert).await?;
                Vec::with_capacity(0)
            }
        })
    }

    /// Find a remote endpoint from agents within arc set.
    async fn find_remote_agent_within_arc(
        &self,
        agent: &Arc<KitsuneAgent>,
        arc_set: Arc<DhtArcSet>,
        local_agents: &HashSet<Arc<KitsuneAgent>>,
        current_rounds: HashSet<Tx2Cert>,
    ) -> KitsuneResult<Option<(GossipTgt, TxUrl)>> {
        // Get the time range for this gossip.
        let mut remote_agents_within_arc_set =
            store::agents_within_arcset(&self.evt_sender, &self.space, &agent, arc_set.clone())
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
                            ra.url_list
                                .iter()
                                .filter_map(|url| {
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
                                        .filter(|(tgt, _)| !current_rounds.contains(tgt.cert()))
                                })
                                .next()
                        }),
                )
            }
            None => Ok(None),
        }
    }
}

impl RoundState {
    fn increment_ops_blooms(&mut self) -> u8 {
        self.num_ops_blooms += 1;
        self.num_ops_blooms
    }
}

/// Time range from now into the past.
/// Start must be < end.
fn time_range(start: Duration, end: Duration) -> Range<u64> {
    let now = SystemTime::now();
    let start = now
        .checked_sub(start)
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|t| t.as_secs())
        .unwrap_or(0);

    let end = now
        .checked_sub(end)
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|t| t.as_secs())
        .unwrap_or(0);

    start..end
}

kitsune_p2p_types::write_codec_enum! {
    /// SimpleBloom Gossip Wire Protocol Codec
    codec ShardedGossipWire {
        /// Initiate a round of gossip with a remote node
        Initiate(0x10) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<ArcInterval>,
            /// A random number to resolve concurrent initiates.
            id.1: u8,
        },

        /// Accept an incoming round of gossip from a remote node
        Accept(0x20) {
            /// The list of arc intervals (equivalent to a [`DhtArcSet`])
            /// for all local agents
            intervals.0: Vec<ArcInterval>,
        },

        /// Send Agent Info Boom
        Agents(0x30) {
            /// The bloom filter for agent data
            filter.0: PoolBuf,
        },

        /// Any agents that were missing from the remote bloom.
        MissingAgents(0x40) {
            /// The missing agents
            agents.0: Vec<Arc<AgentInfoSigned>>,
        },

        /// Send Agent Info Boom
        Ops(0x50) {
            /// The bloom filter for op data
            filter.0: Option<PoolBuf>,
            /// Is this the last bloom to be sent?
            finished.1: bool,
        },

        /// Any ops that were missing from the remote bloom.
        MissingOps(0x60) {
            /// The missing ops
            ops.0: Vec<Arc<(Arc<KitsuneOpHash>, Vec<u8>)>>,
            /// Is this the last chunk of ops to be sent in response
            /// to the bloom filter that we're responding to?
            finished.1: bool,
        },

        /// The node you are trying to gossip with has no agents anymore.
        // maackle: perhaps this should be a flag on the Agents (bloom) message,
        //          since it would still be useful to gossip peer data.
        // FS: No this literally means there's no local agents to gossip with.
        NoAgents(0x80) {
        },
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
        let _ = self.gossip.inner.share_mut(move |i, _| {
            i.local_agents.insert(a);
            Ok(())
        });
    }

    fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        let _ = self.gossip.inner.share_mut(move |i, _| {
            i.local_agents.remove(&a);
            Ok(())
        });
    }

    fn close(&self) {
        todo!()
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

/// Create a [`GossipModuleFactory`]
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
