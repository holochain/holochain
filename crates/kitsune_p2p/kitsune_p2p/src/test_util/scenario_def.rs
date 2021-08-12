//! Declarative definition of multi-conductor sharded scenarios.

use kitsune_p2p_types::dht_arc::{ArcInterval, DhtLocation};
use std::collections::{BTreeSet, HashSet};

/// Abstract representation of the instantaneous state of a sharded network
/// with multiple conductors. Useful for setting up multi-node test scenarios,
/// and for deriving the expected final state after reaching consistency.
///
/// NB: The "reification" of this representation will break a rule:
/// Agent and Op hashes will have manually defined locations which do NOT
/// match the actual hash content, since it is computationally infeasible to
/// search for hashes which match a given location.
/// (By "reification" I mean the actual concrete Nodes which are set up.)
///
/// Thus, rather than dealing with hash types directly, this representation
/// deals only with locations.
///
/// Thus, note that for simplicity's sake, it's impossible to specify two ops
/// at the same location, which is possible in reality, but rare, and should
/// have no bearing on test results. (TODO: test this case separately)
pub struct ScenarioDef<const N: usize> {
    /// The "nodes" (in Holochain, "conductors") participating in this scenario
    pub nodes: [ScenarioDefNode; N],

    /// Specifies which other nodes are present in the peer store of each node.
    /// The array index matches the array defined in `ShardedScenario::nodes`.
    pub peer_matrix: PeerMatrix<N>,

    /// Represents latencies between nodes, to be simulated.
    /// If None, all latencies are zero.
    pub _latency_matrix: LatencyMatrix<N>,
}

impl<const N: usize> ScenarioDef<N> {
    /// Constructor
    pub fn new(nodes: [ScenarioDefNode; N], peer_matrix: PeerMatrix<N>) -> Self {
        Self::new_with_latency(nodes, peer_matrix, None)
    }

    fn new_with_latency(
        nodes: [ScenarioDefNode; N],
        peer_matrix: PeerMatrix<N>,
        _latency_matrix: LatencyMatrix<N>,
    ) -> Self {
        Self {
            nodes,
            peer_matrix,
            _latency_matrix,
        }
    }
}

/// An individual node in a sharded scenario.
/// The only data needed is the list of local agents.
pub struct ScenarioDefNode {
    /// The agents local to this node
    pub agents: HashSet<ScenarioDefAgent>,
}

impl ScenarioDefNode {
    /// Constructor
    pub fn new(agents: HashSet<ScenarioDefAgent>) -> Self {
        Self { agents }
    }
}

/// An individual agent on a node in a sharded scenario
#[derive(PartialEq, Eq, Hash)]
pub struct ScenarioDefAgent {
    /// The storage arc for this agent
    pub arc: ArcInterval,
    /// The ops stored by this agent
    pub ops: BTreeSet<DhtLocation>,
}

impl ScenarioDefAgent {
    /// Constructor
    pub fn new<O: Copy + Into<DhtLocation>>(arc: ArcInterval, ops: &[O]) -> Self {
        Self {
            arc,
            ops: ops.into_iter().copied().map(Into::into).collect(),
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
    /// All nodes know about all other nodes
    Full,
    /// Each index of the matrix is a hashset of other indices: The node at
    /// this index knows about the other nodes at the indices in the hashset.
    Sparse([HashSet<usize>; N]),
}

/// Just construct a scenario to illustrate/experience how it's done
#[test]
fn constructors() {
    use maplit::hashset;
    use ScenarioDefAgent as Agent;
    use ScenarioDefNode as Node;
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
    let _scenario = ScenarioDef::new(nodes, PeerMatrix::Sparse([hashset![1], hashset![]]));
}
