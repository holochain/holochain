use crate::event::{QueryOpHashesEvt, TimeWindow};
use crate::gossip::sharded_gossip::{BandwidthThrottle, GossipType, ShardedGossip};
use crate::test_util::spawn_handler;
use crate::types::gossip::*;
use crate::types::wire;
use futures::stream::StreamExt;
use ghost_actor::GhostResult;
use itertools::Itertools;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::{AgentInfoInner, AgentInfoSigned};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::dht_arc::loc8::Loc8;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArc, DhtLocation};
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinHandle;

use super::switchboard_evt_handler::SwitchboardEventHandler;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

pub type NodeEp = Tx2EpHnd<wire::Wire>;

static ZERO_SPACE: once_cell::sync::Lazy<Arc<KitsuneSpace>> =
    once_cell::sync::Lazy::new(|| Arc::new(KitsuneSpace::new([0; 36].to_vec())));

#[derive(Debug)]
pub struct NodeEntry {
    pub(super) agents: HashMap<Loc8, AgentEntry>,
    pub(super) gossip: GossipModule,
}

impl NodeEntry {
    pub fn agent_by_loc8(&self, loc8: Loc8) -> Option<&AgentEntry> {
        self.agents.get(&loc8)
    }

    pub fn agent_by_hash(&self, hash: &KitsuneAgent) -> Option<&AgentEntry> {
        self.agent_by_loc8(hash.get_loc().into())
    }
}

/// The value of the SwitchboardSpace::agents hashmap
#[derive(Debug, Clone)]
pub struct AgentEntry {
    /// The AgentInfoSigned for this agent
    pub info: AgentInfoSigned,
    /// The ops held by this agent.
    /// Other data for this op can be found in SwitchboardSpace::ops
    pub ops: HashMap<Loc8, AgentOpEntry>,
}

impl AgentEntry {
    pub fn new(info: AgentInfoSigned) -> Self {
        Self {
            info,
            ops: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]

pub struct AgentOpEntry {
    pub is_integrated: bool,
}

/// The value of the SwitchboardSpace::ops hashmap
#[derive(Debug, Clone)]
pub struct OpEntry {
    /// Not strictly necessary as it can be computed from the Loc8 key, but here
    /// for convenience since there is no one-step way to go from Loc8 -> KitsuneOpHash
    pub hash: KOpHash,
    /// The opaque data for the op. Probably doesn't matter and can be removed.
    pub data: Vec<u8>,
    /// The timestamp associated with this op. Same for all agents, intrinsic to the
    /// op itself.
    pub timestamp: Timestamp,
}

pub struct SpaceEntry {
    state: Share<SwitchboardSpace>,
    tasks: Vec<(
        tokio::task::JoinHandle<GhostResult<()>>,
        tokio::task::JoinHandle<KitsuneResult<()>>,
    )>,
}

#[derive(derive_more::AsRef, derive_more::AsMut, derive_more::Deref, derive_more::DerefMut)]
pub struct Switchboard {
    /// Only a single space is currently supported.
    ///
    /// Eventually the switchboard could accomodate multiple spaces if needed.
    /// The spaces will still have to be inside `Share`, because the event handler
    /// for each node in the space needs shared mutable access to the state.
    spaces: HashMap<KSpace, SpaceEntry>,
}

impl Switchboard {
    pub fn new() -> Self {
        // TODO: when supporting multiple spaces, don't add this default space
        // in the constructor.
        let space = ZERO_SPACE.clone();
        let mut spaces = HashMap::new();
        let entry = SpaceEntry {
            state: Share::new(SwitchboardSpace::new(space.clone())),
            tasks: Vec::new(),
        };
        spaces.insert(space, entry);
        Self { spaces }
    }

    // /// Get the hash of the singleton space
    // pub fn space_hash(&self) -> Arc<KitsuneSpace> {
    //     ZERO_SPACE.clone()
    // }

    /// Get the state for the singleton space
    pub fn space_state(&self) -> Share<SwitchboardSpace> {
        self.spaces.get(&**ZERO_SPACE).unwrap().state.clone()
    }

    /// Set up state and handler tasks for a new node in the space
    pub async fn add_node(&mut self, mem_config: MemConfig) -> NodeEp {
        let space = ZERO_SPACE.clone();

        let f = tx2_mem_adapter(mem_config).await.unwrap();
        let f = tx2_pool_promote(f, Default::default());
        let f = tx2_api(f, Default::default());

        let mut ep = f
            .bind("none:", KitsuneTimeout::from_millis(5000))
            .await
            .unwrap();
        let ep_hnd = ep.handle().clone();

        let tuning_params = Arc::new(Default::default());

        let space_state = self.spaces.get(&space).unwrap().state.clone();
        let evt_handler = SwitchboardEventHandler::new(ep_hnd.clone(), space_state.clone());
        let (evt_sender, handler_task) = spawn_handler(evt_handler.clone()).await;

        // TODO: generalize
        let gossip_type = GossipType::Historical;

        let bandwidth = Arc::new(BandwidthThrottle::new(1000.0, 1000.0));

        let gossip = ShardedGossip::new(
            tuning_params,
            space.clone(),
            ep_hnd.clone(),
            evt_sender,
            gossip_type,
            bandwidth,
        );
        let gossip = GossipModule(gossip.clone());
        let gossip2 = gossip.clone();

        let ep_task = metric_task(async move {
            dbg!("begin metric task");
            while let Some(evt) = ep.next().await {
                match dbg!(evt) {
                    // what other messages do i need to handle?
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        match data {
                            wire::Wire::Gossip(wire::Gossip {
                                space: _,
                                data,
                                module,
                            }) => {
                                dbg!(&data, &module);
                                let data: Vec<u8> = data.into();
                                let data: Box<[u8]> = data.into_boxed_slice();

                                gossip2.incoming_gossip(con, url, data)?
                            }
                            _ => unimplemented!(),
                        }
                    }
                    _ => unimplemented!(),
                }
            }
            Ok(())
        });

        // Register the task handles with the space
        self.spaces
            .get_mut(&space)
            .unwrap()
            .tasks
            .push((handler_task, ep_task));

        // Add the node to the space state
        space_state
            .share_mut(|state, _| {
                state.nodes.insert(
                    ep_hnd.clone(),
                    NodeEntry {
                        agents: HashMap::new(),
                        gossip,
                    },
                );
                Ok(())
            })
            .unwrap();

        ep_hnd
    }
}

/// An channel-based implementation of networking for tests, where messages are
/// simply routed in-memory
pub struct SwitchboardSpace {
    space: KSpace,
    pub(super) nodes: HashMap<NodeEp, NodeEntry>,
    pub(super) ops: HashMap<Loc8, OpEntry>,
    metric_tasks: Vec<JoinHandle<KitsuneResult<()>>>,
    handler_tasks: Vec<JoinHandle<ghost_actor::GhostResult<()>>>,
}

impl SwitchboardSpace {
    pub fn new(space: KSpace) -> Self {
        Self {
            space,
            nodes: Default::default(),
            ops: Default::default(),
            metric_tasks: Default::default(),
            handler_tasks: Default::default(),
        }
    }

    pub fn node_for_agent_loc8(&self, loc8: Loc8) -> Option<&NodeEntry> {
        self.nodes
            .values()
            .find(|n| n.agents.keys().contains(&loc8))
    }

    pub fn node_for_agent_loc8_mut(&mut self, loc8: Loc8) -> Option<&mut NodeEntry> {
        self.nodes
            .values_mut()
            .find(|n| n.agents.keys().contains(&loc8))
    }

    pub fn node_for_agent_hash_mut(&mut self, hash: &KitsuneAgent) -> Option<&mut NodeEntry> {
        let agent_loc8 = hash.get_loc().into();
        self.node_for_agent_loc8_mut(agent_loc8)
    }

    /// Look through all nodes for this agent Loc8
    pub fn agent_by_loc8(&self, loc8: Loc8) -> Option<&AgentEntry> {
        self.nodes
            .values()
            .filter_map(|n| n.agent_by_loc8(loc8))
            .next()
    }

    /// Look through all nodes for this agent hash
    pub fn agent_by_hash(&self, hash: &KitsuneAgent) -> Option<&AgentEntry> {
        self.nodes
            .values()
            .filter_map(|n| n.agent_by_hash(hash))
            .next()
    }

    /// Get the agent map for a node. Just for minor boilerplate reduction.
    pub fn agents_for_node(&mut self, node: &NodeEp) -> &mut HashMap<Loc8, AgentEntry> {
        &mut self.nodes.get_mut(node).expect("Node not added").agents
    }

    pub fn add_agent(&mut self, node_ep: &NodeEp, agent_loc8: Loc8, interval: ArcInterval<Loc8>) {
        let agent = agent_from_loc(agent_loc8);
        let info = fake_agent_info(
            self.space.clone(),
            agent.clone(),
            interval.to_dht_location(),
        );
        self.agent_by_loc8(agent_loc8).map(|existing| {
            panic!(
                "Attempted to insert two agents at the same Loc8. Existing agent info: {:?}",
                existing.info
            )
        });
        let node = self
            .nodes
            .get_mut(node_ep)
            .expect("Node must be added first");
        node.agents
            .insert(agent_loc8, AgentEntry::new(info))
            .map(|existing| {
                panic!(
                    "Attempted to insert two agents at the same Loc8. Existing agent info: {:?}",
                    existing.info
                )
            });
        node.gossip.local_agent_join(agent);
    }

    pub fn exchange_peer_info<
        'n,
        L: Borrow<Loc8>,
        A: IntoIterator<Item = L>,
        P: IntoIterator<Item = (&'n NodeEp, A)>,
    >(
        &mut self,
        peers: P,
    ) {
        for (node, agents) in peers.into_iter() {
            let agents: Vec<_> = agents
                .into_iter()
                .map(|loc8| {
                    let loc8: &Loc8 = loc8.borrow();
                    (
                        *loc8,
                        self.node_for_agent_loc8(*loc8)
                            .unwrap()
                            .agents
                            .get(loc8)
                            .unwrap()
                            .to_owned(),
                    )
                })
                .collect();
            self.nodes
                .get_mut(node)
                .expect("No node")
                .agents
                .extend(agents)
        }
    }

    pub fn add_ops_now<O: IntoIterator<Item = Loc8>>(
        &mut self,
        agent_loc: Loc8,
        is_integrated: bool,
        ops: O,
    ) {
        let ops = ops.into_iter().map(|op| (op, Timestamp::now()));
        self.add_ops_timed(agent_loc, is_integrated, ops)
    }

    pub fn add_ops_timed<L: Into<DhtLocation>, O: IntoIterator<Item = (L, Timestamp)>>(
        &mut self,
        agent_loc: L,
        is_integrated: bool,
        ops: O,
    ) {
        let agent = agent_from_loc(agent_loc);

        // Do some pre-computation
        let ops: Vec<_> = ops
            .into_iter()
            .map(|(loc, timestamp)| {
                let loc: DhtLocation = loc.into();
                let loc8: Loc8 = loc.into();
                let hash = op_hash_from_loc(loc8);
                (loc8, hash, timestamp)
            })
            .collect();

        {
            // Update the agent op state, dropping the mutable ref immediately after
            let node = self
                .node_for_agent_hash_mut(&*agent)
                .expect("No agent at this loc8 for node");
            let agent_loc8 = agent.get_loc().into();
            let agent_entry = node.agents.get_mut(&agent_loc8).unwrap();
            for (loc8, _, _) in ops.iter() {
                agent_entry
                    .ops
                    .insert(*loc8, AgentOpEntry { is_integrated });
            }
        }

        // Update node-wide op store with data and timestamp
        for (loc8, hash, timestamp) in ops {
            if let Some(existing) = self.ops.insert(
                loc8,
                OpEntry {
                    hash,
                    data: vec![],
                    timestamp,
                },
            ) {
                panic!(
                    "inserted same op twice. remove this panic if it's not a big deal. {:?}",
                    existing
                );
            }
        }

        // Let gossip module know there's new integrated data now.
        self.node_for_agent_hash_mut(&*agent)
            .expect("No agent at this loc8 for node")
            .gossip
            .new_integrated_data();
    }

    pub fn get_ops_loc8(&mut self, node_ep: &NodeEp) -> Vec<Loc8> {
        let mut ops: Vec<_> = self
            .agents_for_node(node_ep)
            .values()
            .map(|agent| agent.ops.keys())
            .flatten()
            .copied()
            .collect();
        ops.sort();
        ops
    }

    pub fn query_op_hashes(
        &mut self,
        QueryOpHashesEvt {
            space,
            agents,
            window,
            max_ops,
            include_limbo,
        }: QueryOpHashesEvt,
    ) -> Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)> {
        let (ops, timestamps): (Vec<_>, Vec<_>) = self
            .ops
            .iter()
            .filter(|(op_loc8, op)| {
                // Does the op fall within the time window?
                window.contains(&op.timestamp)
                    // Does the op fall within one of the specified arcsets
                    // with the correct integration/limbo criteria?
                        && agents.iter().fold(false, |yes, (agent, arc_set)| {
                            if yes {
                                return true;
                            }
                            arc_set.contains((**op_loc8).into()) &&
                            self.agent_by_hash(&agent)
                                .and_then(|agent| {
                                    agent
                                        .ops
                                        // Does agent hold this op?
                                        .get(op_loc8)
                                        // Does it meet the limbo criteria of the query?
                                        .map(|op| include_limbo || op.is_integrated)
                                })
                                .unwrap_or(false)
                        })
            })
            .map(|(_, op)| (op.hash.clone(), op.timestamp))
            .take(max_ops)
            .unzip();

        if ops.is_empty() {
            None
        } else {
            let window = timestamps
                .into_iter()
                .fold(window, |mut window, timestamp| {
                    if window.start < timestamp {
                        window.start = timestamp;
                    }
                    if window.end > timestamp {
                        window.end = timestamp;
                    }
                    window
                });
            Some((ops, window))
        }
    }
}

fn agent_from_loc<L: Into<DhtLocation>>(loc8: L) -> KAgent {
    let loc: DhtLocation = loc8.into();
    Arc::new(KitsuneAgent::new(loc.to_bytes_36()))
}

fn op_hash_from_loc<L: Into<DhtLocation>>(loc8: L) -> KOpHash {
    let loc: DhtLocation = loc8.into();
    Arc::new(KitsuneOpHash::new(loc.to_bytes_36()))
}

fn fake_agent_info(space: KSpace, agent: KAgent, interval: ArcInterval) -> AgentInfoSigned {
    let state = AgentInfoInner {
        space,
        agent,
        storage_arc: DhtArc::from_interval(interval),
        url_list: vec![],
        signed_at_ms: 0,
        expires_at_ms: 0,
        signature: Arc::new(KitsuneSignature(vec![])),
        encoded_bytes: Box::new([]),
    };
    AgentInfoSigned(Arc::new(state))
}

#[test]
fn hash_from_loc8_roundtrip() {
    for i in [0, 1, -1, i8::MIN, i8::MAX] {
        let i: Loc8 = i;
        assert_eq!(Loc8::from(agent_from_loc(i).get_loc()), i);
        assert_eq!(Loc8::from(op_hash_from_loc(i).get_loc()), i);
    }
}
