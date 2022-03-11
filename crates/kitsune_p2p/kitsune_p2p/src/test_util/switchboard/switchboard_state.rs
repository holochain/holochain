//! An in-memory network for sharded kitsune tests.

use crate::gossip::sharded_gossip::{BandwidthThrottle, GossipType, ShardedGossip};
use crate::test_util::spawn_handler;
use crate::types::gossip::*;
use crate::types::wire;
use crate::HostStub;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use ghost_actor::GhostResult;
use itertools::Itertools;
use kitsune_p2p_proxy::tx2::{tx2_proxy, ProxyConfig};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::agent_info_helper::{AgentInfoEncode, AgentMetaInfoEncode};
use kitsune_p2p_types::agent_info::{AgentInfoInner, AgentInfoSigned};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dht_arc::loc8::Loc8;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArc, DhtLocation};
use kitsune_p2p_types::metrics::metric_task;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinHandle;

use super::switchboard_evt_handler::SwitchboardEventHandler;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

/// Type alias for an endpoint handle
pub type NodeEp = Tx2EpHnd<wire::Wire>;

/// We only implement the switchboard for a singleton space for now, and this
/// is it.
static ZERO_SPACE: once_cell::sync::Lazy<Arc<KitsuneSpace>> =
    once_cell::sync::Lazy::new(|| Arc::new(KitsuneSpace::new([0; 36].to_vec())));

/// Wrapper around the shared state for a Switchboard network. This state
/// represents the shared state across all nodes in a space.
///
/// This is essentially an Arc<Clone<SwitchboardState>>, which is passed to
/// SwitchboardEventHandler and is alsoaccessible to your test, so that you can
/// manually modify state while gossip is modifying the same state.
///
/// When calling `add_node(s)`, a new gossip module is created, a task is
/// spawned to feed incoming network messages to the gossip module, and another
/// task is spawned which handles the events received, mutating the state
/// in the process.
#[derive(Clone)]
pub struct Switchboard {
    inner: Share<SwitchboardState>,
    gossip_type: GossipType,
}

impl Switchboard {
    /// Constructor. Only works for one GossipType at a time.
    // TODO: if it's desirable to test multiple gossip loops running at the
    //   same time on the same state, another method could be exposed to take
    //   an already instantiated `Share<SwitchboardState>`, which will cause
    //   both gossip loops to share the same state.
    //   Or, this could be modified to take a list of GossipTypes, so that
    //   multiple loops will be created internally.
    pub fn new(gossip_type: GossipType) -> Self {
        Self {
            inner: Share::new(SwitchboardState::default()),
            gossip_type,
        }
    }

    /// Simple infallible combinator around `share_mut`, more convenient
    /// for tests.
    pub fn share<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut SwitchboardState) -> R,
    {
        self.inner.share_mut(|sb, _| Ok(f(sb))).unwrap()
    }

    /// Convenience for creating multiple nodes at once, easily destructurable
    /// using array syntax:
    ///
    /// ```ignore
    /// // no need to specify the number, the destructuring will deduce that.
    /// let [n1, n2, n3] = sb.add_nodes();
    /// ```
    pub async fn add_nodes<const N: usize>(
        &self,
        tuning_params: KitsuneP2pTuningParams,
    ) -> [NodeEp; N] {
        let mut nodes = vec![];
        for _ in 0..N {
            nodes.push(self.add_node(tuning_params.clone()).await);
        }
        nodes.try_into().unwrap()
    }

    /// Set up state and handler tasks for a new node in the space
    pub async fn add_node(&self, tuning_params: KitsuneP2pTuningParams) -> NodeEp {
        let space = ZERO_SPACE.clone();

        let mem_config = MemConfig::default();
        let proxy_config = ProxyConfig::default();
        // proxy_config.allow_proxy_fwd = true;

        let f = tx2_mem_adapter(mem_config).await.unwrap();
        let f = tx2_pool_promote(f, tuning_params.clone());
        // Proxy wrapping is needed because sharded_gossip expects it
        let f = tx2_proxy(f, proxy_config).unwrap();
        let f = tx2_api(f, Default::default());

        let mut ep = f
            .bind("none:", KitsuneTimeout::from_millis(5000))
            .await
            .unwrap();
        let ep_hnd = ep.handle().clone();

        let evt_handler = SwitchboardEventHandler::new(ep_hnd.clone(), self.clone());
        let host = HostStub::new();
        let (evt_sender, handler_task) = spawn_handler(evt_handler.clone()).await;

        let bandwidth = Arc::new(BandwidthThrottle::new(1000.0, 1000.0));

        let gossip = ShardedGossip::new(
            tuning_params,
            space.clone(),
            ep_hnd.clone(),
            evt_sender,
            host,
            self.gossip_type,
            bandwidth,
            Default::default(),
        );
        let gossip = GossipModule(gossip);
        let gossip2 = gossip.clone();

        let ep_task = metric_task(async move {
            dbg!("begin metric task");
            while let Some(evt) = ep.next().await {
                match evt {
                    // what other messages do i need to handle?
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        match data {
                            wire::Wire::Gossip(wire::Gossip {
                                space: _,
                                data,
                                module: _,
                            }) => {
                                let data: Vec<u8> = data.into();
                                let data: Box<[u8]> = data.into_boxed_slice();

                                gossip2.incoming_gossip(con, url, data)?
                            }
                            _ => unimplemented!(),
                        }
                    }
                    _evt => {
                        // NB: all other events are ignored
                    }
                }
            }
            Ok(())
        });

        // Register the task handles with the space
        self.share(|sb| {
            sb.metric_tasks.push(ep_task);
            sb.handler_tasks.push(handler_task);

            // Add the node to the space state
            sb.nodes.insert(
                ep_hnd.clone(),
                NodeEntry {
                    local_agents: HashMap::new(),
                    remote_agents: HashMap::new(),
                    ops: HashMap::new(),
                    gossip,
                },
            );
        });

        ep_hnd
    }
}

/// The state of the switchboard, which represents the persisted state of all
/// nodes in a space. These methods may be called directly in your test, after
/// getting a lock on the state via `Switchboard::share`. This same state is
/// modified directly by an actively running GossipModule which is processing
/// messages from other nodes.
pub struct SwitchboardState {
    space: KSpace,
    pub(super) nodes: HashMap<NodeEp, NodeEntry>,
    pub(super) ops: HashMap<Loc8, OpEntry>,
    metric_tasks: Vec<JoinHandle<KitsuneResult<()>>>,
    handler_tasks: Vec<JoinHandle<ghost_actor::GhostResult<()>>>,
}

impl Default for SwitchboardState {
    fn default() -> Self {
        Self {
            space: ZERO_SPACE.clone(),
            nodes: HashMap::new(),
            ops: HashMap::new(),
            metric_tasks: Vec::new(),
            handler_tasks: Vec::new(),
        }
    }
}

impl SwitchboardState {
    /// Add a local agent to the specified node.
    pub fn add_local_agent(&mut self, node_ep: &NodeEp, agent: &SwitchboardAgent) {
        let SwitchboardAgent {
            loc: loc8,
            initial_arc,
        } = agent.clone();
        let agent = agent_from_loc(loc8);
        let info = fake_agent_info(
            self.space.clone(),
            node_ep,
            agent.clone(),
            initial_arc.canonical(),
        );
        if let Some(existing) = self.local_agent_by_loc8(loc8) {
            panic!(
                "Attempted to insert two agents at the same Loc8. Existing agent info: {:?}",
                existing.info
            );
        }
        let node = self
            .nodes
            .get_mut(node_ep)
            .expect("Node must be added first");
        if let Some(existing) = node.local_agents.insert(loc8, AgentEntry::new(info)) {
            panic!(
                "Attempted to insert two agents at the same Loc8. Existing agent info: {:?}",
                existing.info
            );
        }
        node.gossip.local_agent_join(agent);
    }

    /// Helpful ascii visualization of each agent's storage arc coverage
    /// across all nodes.
    pub fn print_ascii_arcs(&self, width: usize, with_ops: bool) {
        const NUM_TICKS: usize = 4;

        // Add numbers at every quarter mark
        let mut spaces = " ".repeat(width);
        for i in 0..NUM_TICKS {
            let n = i * 256 / NUM_TICKS;
            let t = n * width / 256;
            let s = n.to_string();
            spaces.replace_range(t..t + s.len(), &s);
        }

        // Add dots at every eighth mark
        let mut subticks = " ".repeat(width);
        for i in 0..NUM_TICKS * 2 {
            let t = i * width / (NUM_TICKS * 2);
            subticks.replace_range(t..=t, ".");
        }
        subticks.replace_range(width - 1..width, ".");

        println!("node agent  {}   mid bounds", spaces);
        println!("            {}", subticks);
        let mut nodes: Vec<_> = self.nodes.iter().collect();
        nodes.sort_by_key(|(ep, _)| ep.uniq());
        for (ep, node) in nodes.into_iter() {
            let node_id = ep.uniq();
            let ascii = if with_ops {
                let ops = node.ops.keys().copied();
                ArcInterval::Empty.to_ascii_with_ops(width, ops)
            } else {
                ArcInterval::Empty.to_ascii(width)
            };
            println!(
                "{:>4} {:>+5} ({:^width$})",
                node_id,
                "",
                ascii,
                width = width
            );
            for (agent_loc8, agent) in node.local_agents.iter() {
                let interval = agent.info.storage_arc.interval();
                let ascii = interval.to_ascii(width);
                println!(
                    "{:>4} {:>+5} |{:^width$}| {:>+4} {:?}",
                    "",
                    agent_loc8,
                    ascii,
                    interval.start_loc().as_loc8(),
                    interval.map(|b| DhtLocation::as_loc8(&b)),
                    width = width
                );
            }
        }
    }

    /// Print the list of peers recorded in each node
    pub fn print_peer_lists(&self) {
        println!("node agents");
        let mut nodes: Vec<_> = self.nodes.iter().collect();
        nodes.sort_by_key(|(ep, _)| ep.uniq());
        for (ep, node) in nodes.iter() {
            let mut agent_locs: Vec<_> = node.all_agent_locs().into_iter().collect();
            agent_locs.sort();
            println!("{:>4} {:?}", ep.uniq(), agent_locs);
        }
    }

    /// List all peer info (local and remote agents) that this node knows about.
    pub fn all_peers(&mut self, node: &NodeEp) -> Vec<Loc8> {
        let mut locs: Vec<_> = self
            .nodes
            .get(node)
            .unwrap()
            .all_agent_locs()
            .into_iter()
            .collect();
        locs.sort();
        locs
    }

    /// Inject the agent info from the specified agents into the specified
    /// node's remote agent store.
    ///
    /// This is used to set up arbitrary situations where not every peer
    /// knows about every other peer.
    pub fn inject_peer_info<L: AsRef<Loc8>, A: IntoIterator<Item = L>>(
        &mut self,
        node: &NodeEp,
        agents: A,
    ) {
        let agents: Vec<_> = agents
            .into_iter()
            .map(|loc| {
                let loc8 = *loc.as_ref();
                (
                    loc8,
                    self.node_for_local_agent_loc8(loc8)
                        .unwrap()
                        .local_agents
                        .get(&loc8)
                        .unwrap()
                        .info
                        .to_owned(),
                )
            })
            .collect();
        self.nodes
            .get_mut(node)
            .expect("No node")
            .remote_agents
            .extend(agents)
    }

    /// Let every node know about every other agent.
    ///
    /// This is not realistic, but useful as a convenience when not needing to
    /// explicitly test peer gossip.
    pub fn exchange_all_peer_info(&mut self) {
        let all_agent_locs: Vec<_> = self
            .nodes
            .values()
            .flat_map(|n| n.local_agents.keys())
            .collect();
        let info: Vec<(_, Vec<_>)> = self
            .nodes
            .iter()
            .map(|(ep, n)| {
                let local: HashSet<_> = n.local_agents.keys().collect();
                (
                    ep.clone(),
                    all_agent_locs
                        .iter()
                        .filter(|loc| !local.contains(**loc))
                        .copied()
                        .copied()
                        // box it to satisfy AsRef<Loc8>
                        .map(Box::new)
                        .collect(),
                )
            })
            .collect();
        for (node, agents) in info {
            self.inject_peer_info(&node, agents);
        }
    }

    /// Add ops by Loc8 location, specifying the Timestamp that each was created.
    /// Each Loc8 becomes a new Op added to an agent's op store.
    pub fn add_ops_timed<L: Into<Loc8>, O: IntoIterator<Item = (L, Timestamp)>>(
        &mut self,
        node_ep: &NodeEp,
        is_integrated: bool,
        ops: O,
    ) {
        // Do some pre-computation
        let ops: Vec<_> = ops
            .into_iter()
            .map(|(l, timestamp)| {
                let loc8: Loc8 = l.into();
                let hash = op_hash_from_loc(loc8);
                (loc8, hash, timestamp)
            })
            .collect();

        let node = self.nodes.get_mut(node_ep).expect("No node");
        for (loc8, _, _) in ops.iter() {
            node.ops.insert(*loc8, NodeOpEntry { is_integrated });
        }

        // Update node-wide op store with data and timestamp
        for (loc8, hash, timestamp) in ops {
            if let Some(existing) = self.ops.insert(loc8, OpEntry { hash, timestamp }) {
                tracing::warn!(
                    "inserted same op twice. this could be significant if dealing with custom timestamps. {:?}",
                    existing
                );
            }
        }

        // Let gossip module know there's new integrated data now.
        node.gossip.new_integrated_data();
    }

    /// Convenient counterpart to `add_ops_timed`, causes each op to be added
    /// at the current system time.
    pub fn add_ops_now<L: Into<Loc8>, O: IntoIterator<Item = L>>(
        &mut self,
        node_ep: &NodeEp,
        is_integrated: bool,
        ops: O,
    ) {
        let ops = ops.into_iter().map(|op| (op, Timestamp::now()));
        self.add_ops_timed(node_ep, is_integrated, ops)
    }

    /// Get all ops held by a node in terms of their Loc8 location.
    ///
    /// Use this to make assertions about what ops are held after gossip has run.
    pub fn get_ops_loc8(&mut self, node_ep: &NodeEp) -> BTreeSet<Loc8> {
        self.nodes
            .get(node_ep)
            .unwrap()
            .ops
            .keys()
            .copied()
            .collect()
    }

    pub(super) fn node_for_local_agent_loc8(&self, loc8: Loc8) -> Option<&NodeEntry> {
        self.nodes
            .values()
            .find(|n| n.local_agents.keys().contains(&loc8))
    }

    pub(super) fn node_for_local_agent_loc8_mut(&mut self, loc8: Loc8) -> Option<&mut NodeEntry> {
        self.nodes
            .values_mut()
            .find(|n| n.local_agents.keys().contains(&loc8))
    }

    pub(super) fn node_for_local_agent_hash_mut(
        &mut self,
        hash: &KitsuneAgent,
    ) -> Option<&mut NodeEntry> {
        let agent_loc8 = hash.get_loc().as_loc8();
        self.node_for_local_agent_loc8_mut(agent_loc8)
    }

    /// Look through all nodes for this agent Loc8
    pub(super) fn local_agent_by_loc8(&self, loc8: Loc8) -> Option<&AgentEntry> {
        self.nodes
            .values()
            .filter_map(|n| n.local_agent_by_loc8(loc8))
            .next()
    }

    /// Look through all nodes for this agent hash
    pub(super) fn local_agent_by_hash(&self, hash: &KitsuneAgent) -> Option<&AgentEntry> {
        self.nodes
            .values()
            .filter_map(|n| n.local_agent_by_hash(hash))
            .next()
    }

    /// Get the local agent map for a node. Just for minor boilerplate reduction.
    pub(super) fn local_agents_for_node(
        &mut self,
        node: &NodeEp,
    ) -> &mut HashMap<Loc8, AgentEntry> {
        &mut self
            .nodes
            .get_mut(node)
            .expect("Node not added")
            .local_agents
    }

    /// Get the remote agent map for a node. Just for minor boilerplate reduction.
    pub(super) fn remote_agents_for_node(
        &mut self,
        node: &NodeEp,
    ) -> &mut HashMap<Loc8, AgentInfoSigned> {
        &mut self
            .nodes
            .get_mut(node)
            .expect("Node not added")
            .remote_agents
    }
}

/// Representation of an agent on a Switchboard node.
///
/// The reason for this type is to reduce the redundancy
/// of needing to specify both the agent's location and the
/// storage arc. It is most convenient to specify arcs in terms
/// of ArcInterval, but we use the agent's Loc8 location as their
/// unique identifier. This allows specification of agents in
/// terms of arcs.
#[derive(Clone, derive_more::AsRef)]
pub struct SwitchboardAgent {
    pub(super) loc: Loc8,
    initial_arc: ArcInterval<Loc8>,
}

impl SwitchboardAgent {
    /// Construct an agent with a full arc at the specified location.
    pub fn full<L: Into<Loc8>>(loc: L) -> Self {
        Self {
            loc: loc.into(),
            initial_arc: ArcInterval::Full,
        }
    }

    /// Construct an agent from arc bounds.
    /// The agent's location is taken as the midpoint of the arc.
    pub fn from_bounds<L: Into<Loc8>>(lo: L, hi: L) -> Self {
        let lo: Loc8 = lo.into();
        let hi: Loc8 = hi.into();
        let initial_arc = ArcInterval::Bounded(lo, hi);
        let loc8 = initial_arc.clone().canonical().start_loc().as_loc8();

        Self {
            loc: loc8,
            initial_arc,
        }
    }

    /// Construct an agent from arc bounds.
    /// The agent's location is taken as the midpoint of the arc.
    pub fn from_start_and_half_len<L: Into<Loc8>>(start: L, half_len: u8) -> Self {
        let start: Loc8 = start.into();
        let half_len = Loc8::upscale(half_len as i32);
        let initial_arc = DhtArc::new(start, half_len).interval().as_loc8();

        Self {
            loc: start,
            initial_arc,
        }
    }
}

/// The value of the Switchboard::spaces hashmap
#[allow(clippy::type_complexity)]
pub struct SpaceEntry {
    state: Share<SwitchboardState>,
    tasks: Vec<(
        tokio::task::JoinHandle<GhostResult<()>>,
        tokio::task::JoinHandle<KitsuneResult<()>>,
    )>,
}

/// The value of the SwitchboardSpace::nodes hashmap
#[derive(Debug)]
pub struct NodeEntry {
    pub(super) local_agents: HashMap<Loc8, AgentEntry>,
    pub(super) remote_agents: HashMap<Loc8, AgentInfoSigned>,
    /// The ops held by this node.
    /// Other data for this op can be found in SwitchboardSpace::ops
    pub(super) ops: HashMap<Loc8, NodeOpEntry>,
    pub(super) gossip: GossipModule,
}

impl NodeEntry {
    pub(super) fn local_agent_by_loc8(&self, loc8: Loc8) -> Option<&AgentEntry> {
        self.local_agents.get(&loc8)
    }

    pub(super) fn local_agent_by_loc8_mut(&mut self, loc8: Loc8) -> Option<&mut AgentEntry> {
        self.local_agents.get_mut(&loc8)
    }

    pub(super) fn local_agent_by_hash(&self, hash: &KitsuneAgent) -> Option<&AgentEntry> {
        self.local_agent_by_loc8(hash.get_loc().as_loc8())
    }

    pub(super) fn local_agent_by_hash_mut(
        &mut self,
        hash: &KitsuneAgent,
    ) -> Option<&mut AgentEntry> {
        self.local_agent_by_loc8_mut(hash.get_loc().as_loc8())
    }

    pub(super) fn all_agent_infos(&self) -> HashSet<AgentInfoSigned> {
        self.local_agents
            .values()
            .map(|a| a.info.clone())
            .chain(self.remote_agents.values().cloned())
            .collect()
    }

    pub(super) fn all_agent_locs(&self) -> HashSet<Loc8> {
        self.all_agent_infos()
            .into_iter()
            .map(|info| info.to_agent_arc().0.get_loc().as_loc8())
            .collect()
    }
}

/// The value of the SwitchboardSpace::agents hashmap
#[derive(Debug, Clone)]
pub struct AgentEntry {
    /// The AgentInfoSigned for this agent
    pub info: AgentInfoSigned,
}

impl AgentEntry {
    /// Constructor, initialized with empty op list
    pub fn new(info: AgentInfoSigned) -> Self {
        Self { info }
    }
}

/// The value of the AgentEntry::ops HashMap
#[derive(Debug, Clone)]
pub struct NodeOpEntry {
    /// Whether the op should be treated as "integrated".
    /// NB: this is a new concept to kitsune, only implicitly hinted at by the
    /// `include_limbo` option for op fetches. "Limbo" implies "not integrated".
    pub is_integrated: bool,
}

/// The value of the SwitchboardSpace::ops hashmap
///
/// Note that in a real implementation, the op store would include the actual
/// op data. Since op data is opaque to kitsune, we don't need to actually store
/// it for these tests and can just use dummy values. *Actually*, we take
/// take advantage of that fact by hijacking the op data to include a single
/// byte which represents the Loc8 location of this op.
#[derive(Debug, Clone)]
pub struct OpEntry {
    /// Not strictly necessary as it can be computed from the Loc8 key, but here
    /// for convenience
    pub hash: KOpHash,
    /// The timestamp associated with this op. Same for all agents, intrinsic to the
    /// op itself.
    pub timestamp: Timestamp,
}

fn agent_from_loc<L: Into<DhtLocation>>(loc8: L) -> KAgent {
    let loc: DhtLocation = loc8.into();
    Arc::new(KitsuneAgent::new(loc.to_representative_test_bytes_36()))
}

fn op_hash_from_loc<L: Into<DhtLocation>>(loc8: L) -> KOpHash {
    let loc: DhtLocation = loc8.into();
    Arc::new(KitsuneOpHash::new(loc.to_representative_test_bytes_36()))
}

fn fake_agent_info(
    space: KSpace,
    node: &NodeEp,
    agent: KAgent,
    interval: ArcInterval,
) -> AgentInfoSigned {
    use crate::fixt::*;
    let url_list = vec![node.local_addr().unwrap()];
    let meta_info = AgentMetaInfoEncode {
        dht_storage_arc_half_length: 0,
    };
    let mut buf = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut buf, meta_info).unwrap();
    let meta_info = buf.into_boxed_slice();

    let info = AgentInfoEncode {
        space: space.clone(),
        agent: agent.clone(),
        urls: url_list.clone(),
        signed_at_ms: 0,
        expires_after_ms: u64::MAX,
        meta_info,
    };
    let mut buf = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut buf, info).unwrap();
    let encoded_bytes = buf.into_boxed_slice();
    let state = AgentInfoInner {
        space,
        agent,
        storage_arc: DhtArc::from_interval(interval),
        url_list,
        signed_at_ms: 0,
        expires_at_ms: u64::MAX,
        signature: Arc::new(fixt::prelude::fixt!(KitsuneSignature)),
        encoded_bytes,
    };
    AgentInfoSigned(Arc::new(state))
}

#[test]
fn hash_from_loc8_roundtrip() {
    for i in [0, 1, -1, -128, 127] {
        let i: Loc8 = i.into();
        assert_eq!(agent_from_loc(i).get_loc().as_loc8(), i);
        assert_eq!(op_hash_from_loc(i).get_loc().as_loc8(), i);
    }
    for i in [0, 1, 254, 255, 127, 128] {
        let i: Loc8 = i.into();
        assert_eq!(agent_from_loc(i).get_loc().as_loc8(), i);
        assert_eq!(op_hash_from_loc(i).get_loc().as_loc8(), i);
    }
}
