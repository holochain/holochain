use kitsune_p2p_types::{
    dht_arc::{ArcInterval, DhtLocation},
    Tx2Cert,
};
use maplit::hashset;
use observability::tracing::Instrument;

use crate::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

/// Concise representation of data held by various agents in a sharded scenario,
/// without having to refer to explicit op hashes or locations.
///
/// This type is intended to be used to easily define arbitrary sharded network scenarios,
/// to test various cases of local sync and gossip. It's expected that we'll eventually have a
/// small library of such scenarios, defined in terms of this type.
///
/// See [`mock_agent_persistence`] for usage detail.
pub struct OwnershipData {
    /// Total number of op hashes to be generated
    pub total_ops: usize,
    /// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
    pub agents: Vec<OwnershipDataAgent>,
}

impl OwnershipData {
    /// Construct `OwnershipData` from a more compact "untagged" format using
    /// tuples instead of structs. This is intended to be the canonical constructor.
    pub fn from_compact(total_ops: usize, v: Vec<OwnershipDataAgentCompact>) -> Self {
        Self {
            total_ops,
            agents: v
                .into_iter()
                .map(|(agent, arc_indices, hash_indices)| OwnershipDataAgent {
                    agent,
                    arc_indices,
                    hash_indices,
                })
                .collect(),
        }
    }
}

/// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
pub struct OwnershipDataAgent {
    /// The agent in question
    pub agent: Arc<KitsuneAgent>,
    /// The start and end indices of the arc for this agent
    pub arc_indices: (usize, usize),
    /// The indices of ops to consider as owned
    pub hash_indices: Vec<usize>,
}

/// Same as [`OwnershipDataAgent`], but using a tuple instead of a struct.
/// It's just more compact.
pub type OwnershipDataAgentCompact = (Arc<KitsuneAgent>, (usize, usize), Vec<usize>);

/// Abstract representation of the instantaneous state of a sharded network
/// with multiple conductors.
///
/// NB: The reified form of this representation will break a rule:
/// Agent and Op hashes will have manually defined locations which do NOT
/// match the actual hash content.
///
/// Thus, rather than dealing with hash types directly, this representation
/// deals only with locations.
///
/// Also, note that for simplicity's sake, it's impossible to specify two ops
/// at the same location, which is possible in reality, but rare, and should
/// have no bearing on test results. (TODO: test this case separately)
pub struct ShardedScenario<const N: usize> {
    /// The locations of all ops held by all agents
    ops: BTreeSet<DhtLocation>,

    /// The "nodes" (in Holochain, "conductors") participating in this scenario
    nodes: [ShardedScenarioNode; N],

    /// Specifies which other nodes are present in the peer store of each node.
    /// The array index matches the array defined in `ShardedScenario::nodes`.
    peer_matrix: PeerMatrix<N>,

    /// Represents latencies between nodes, to be simulated.
    /// If None, all latencies are zero.
    latency_matrix: LatencyMatrix<N>,
}

impl<const N: usize> ShardedScenario<N> {
    pub fn new<O: Copy + Into<DhtLocation>>(
        ops: &[O],
        nodes: [ShardedScenarioNode; N],
        peer_matrix: PeerMatrix<N>,
        latency_matrix: LatencyMatrix<N>,
    ) -> Self {
        Self {
            ops: ops.into_iter().copied().map(Into::into).collect(),
            nodes,
            peer_matrix,
            latency_matrix,
        }
    }
}

/// An individual node in a sharded scenario.
/// The only data needed is the list of local agents.
pub struct ShardedScenarioNode(HashSet<ShardedScenarioAgent>);

impl ShardedScenarioNode {
    pub fn new(agents: HashSet<ShardedScenarioAgent>) -> Self {
        Self(agents)
    }
}

/// An individual agent on a node in a sharded scenario
#[derive(PartialEq, Eq, Hash)]
pub struct ShardedScenarioAgent {
    /// The storage arc for this agent
    arc: ArcInterval,
    /// The ops stored by this agent
    ops_held: BTreeSet<DhtLocation>,
}

impl ShardedScenarioAgent {
    pub fn new<O: Copy + Into<DhtLocation>>(arc: ArcInterval, ops_held: &[O]) -> Self {
        Self {
            arc,
            ops_held: ops_held.into_iter().copied().map(Into::into).collect(),
        }
    }
}

/// A latency matrix, defining a simulated latency between any two nodes,
/// i.e. latency_matrix[A][B] is the latency in milliseconds for communication
/// from node A to node B.
/// To represent partitions, just set the latency very high (`u32::MAX`).
/// If None, all latencies are zero.
pub type LatencyMatrix<const N: usize> = Option<[[u32; N]; N]>;

/// Specifies which other nodes are present in the peer store of each node.
/// The array index matches the array defined in `ShardedScenario::nodes`.
pub enum PeerMatrix<const N: usize> {
    Full,
    Sparse([HashSet<usize>; N]),
}

/// Just construct a scenario to illustrate/experience how it's done
#[test]
fn constructors() {
    use ShardedScenarioAgent as Agent;
    use ShardedScenarioNode as Node;
    let ops: Vec<DhtLocation> = (-10..11).map(i32::into).collect();
    let nodes = [
        Node::new(hashset![
            Agent::new(
                ArcInterval::Bounded(ops[0].into(), ops[2].into()),
                &[ops[0], ops[1]],
            ),
            Agent::new(
                ArcInterval::Bounded(ops[3].into(), ops[4].into()),
                &[ops[3], ops[4]],
            ),
        ]),
        Node::new(hashset![
            Agent::new(
                ArcInterval::Bounded(ops[0].into(), ops[2].into()),
                &[ops[5], ops[7]],
            ),
            Agent::new(
                ArcInterval::Bounded(ops[3].into(), ops[4].into()),
                &[ops[6], ops[9]],
            ),
        ]),
    ];
    let _scenario = ShardedScenario::new(
        ops.as_slice(),
        nodes,
        PeerMatrix::Sparse([hashset![1], hashset![]]),
        Some([[0, 250], [250, 0]]),
    );
}
