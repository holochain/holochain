//! Declarative definition of multi-conductor sharded scenarios.

use std::collections::{BTreeSet, HashSet};

use kitsune_p2p_types::dht_arc::ArcInterval;

/// The total number of items in a collection. This is set to 256 for convenience,
/// but could be changed, or could make this a parameter on a per-collection basis
const TOTAL: usize = u8::MAX as usize + 1;
/// The size of each "bucket" represented by each item.
const BUCKET_SIZE: usize = ((u32::MAX as u64 + 1) / (TOTAL as u64)) as usize;

/// A "coarse" DHT location specification, defined at a lower resolution
/// than the full u32 space, for convenience in more easily covering the entire
/// space in tests.
pub type CoarseLoc = i8;

/// Abstract representation of the instantaneous state of a sharded network
/// with multiple conductors. Useful for setting up multi-node test scenarios,
/// and for deriving the expected final state after reaching consistency.
///
/// NB: The concrete scenarios derived from this definition will in general break some rules:
///   - The agent arcs will not be centered on the agent's DHT location.
///   - The authors in the database will not match the Headers created.
///     - this means that two agents could claim authorship over the same ops!
///
/// Thus, rather than dealing with hash types directly, this representation
/// deals only with locations.
///
/// Thus, note that for simplicity's sake, it's impossible to specify two ops
/// at the same location, which is possible in reality, but rare, and should
/// have no bearing on test results. (TODO: test this case separately)
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
pub struct ScenarioDefNode {
    /// The agents local to this node
    pub agents: HashSet<ScenarioDefAgent>,
}

impl ScenarioDefNode {
    /// Constructor
    pub fn new<A: IntoIterator<Item = ScenarioDefAgent>>(agents: A) -> Self {
        Self {
            agents: agents.into_iter().collect(),
        }
    }
}

/// An individual agent on a node in a sharded scenario
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ScenarioDefAgent {
    /// The storage arc for this agent
    arc: (CoarseLoc, CoarseLoc),
    /// The ops stored by this agent
    pub ops: BTreeSet<CoarseLoc>,
}

impl ScenarioDefAgent {
    /// Constructor
    pub fn new<O: IntoIterator<Item = CoarseLoc>>(arc: (CoarseLoc, CoarseLoc), ops: O) -> Self {
        let ops: BTreeSet<CoarseLoc> = ops.into_iter().collect();
        assert!(
            ops.len() > 0,
            "Must provide at least one op per Agent, so that a chain head can be determined"
        );
        Self { arc, ops }
    }

    /// Produce an ArcInterval in the u32 space from the lower-resolution
    /// definition, based on the resolution defined in the ScenarioDef which
    /// is passed in
    pub fn arc(&self) -> ArcInterval {
        let start = rectify_index(TOTAL, self.arc.0);
        let end = rectify_index(TOTAL, self.arc.1 + 1) - 1;
        ArcInterval::new(start, end)
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
#[derive(Debug, PartialEq, Eq)]
pub enum PeerMatrix<const N: usize> {
    /// All nodes know about all other nodes
    Full,
    /// Each index of the matrix is a hashset of other indices: The node at
    /// this index knows about the other nodes at the indices in the hashset.
    Sparse([HashSet<usize>; N]),
}

impl<const N: usize> PeerMatrix<N> {
    /// Construct a full matrix (full peer connectivity)
    pub fn full() -> Self {
        Self::Full
    }

    /// Construct a sparse matrix by the given nodes.
    /// More convenient than constructing the enum variant directly, since the
    /// inner collection type is a slice rather than a HashSet.
    pub fn sparse<'a>(matrix: [&'a [usize]; N]) -> Self {
        use std::convert::TryInto;
        Self::Sparse(
            matrix
                // TODO: when array map stabilizes, the node.clone() below
                // can be removed
                .iter()
                .map(|node| {
                    node.clone()
                        .into_iter()
                        .map(|u| u.clone())
                        .collect::<HashSet<_>>()
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
    }
}

/// Map a signed index into an unsigned index
pub fn rectify_index(num: usize, i: i8) -> u32 {
    if i < 0 {
        (num as isize + i as isize) as u32
    } else {
        i as u32
    }
}

/// Just construct a scenario to illustrate/experience how it's done
#[test]
fn constructors() {
    use ScenarioDefAgent as Agent;
    use ScenarioDefNode as Node;
    let ops: Vec<CoarseLoc> = (-10..11).map(i8::into).collect();
    let nodes = [
        Node::new([
            Agent::new((ops[0], ops[2]), [ops[0], ops[1]]),
            Agent::new((ops[3], ops[4]), [ops[3], ops[4]]),
        ]),
        Node::new([
            Agent::new((ops[0], ops[2]), [ops[5], ops[7]]),
            Agent::new((ops[3], ops[4]), [ops[6], ops[9]]),
        ]),
    ];
    let _scenario = ScenarioDef::new(nodes, PeerMatrix::sparse([&[1], &[]]));
}
