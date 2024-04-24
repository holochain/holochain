use crate::sweettest::sweet_topos::edge::NetworkTopologyEdge;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use crate::sweettest::SweetConductor;
use crate::sweettest::SweetConductorConfig;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use async_once_cell::OnceCell;
use contrafact::MutationError;
use holochain_conductor_api::config::conductor::KeystoreConfig;
use holochain_types::prelude::DnaFile;
use holochain_util::tokio_helper;
use petgraph::algo::connected_components;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use petgraph::unionfind::UnionFind;
use petgraph::visit::NodeIndexable;
use rand::seq::SliceRandom;
use shrinkwraprs::Shrinkwrap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Some orphan rule hoop jumping.
#[derive(Clone, Debug, Default)]
pub struct NetworkTopologyConductor(Arc<OnceCell<RwLock<SweetConductor>>>);

impl PartialEq for NetworkTopologyConductor {
    fn eq(&self, other: &Self) -> bool {
        tokio_helper::block_forever_on(async {
            // We're going to ensure the conductors are initialized, then read
            // their IDs and compare them. Comparing uninitialized once cells
            // seems pointless so we don't do it.
            let self_id = self.lock().await.read().await.id();
            let other_id = other.lock().await.read().await.id();
            self_id == other_id
        })
    }
}

impl Eq for NetworkTopologyConductor {}

impl NetworkTopologyConductor {
    /// Build a new network topology conductor. Actually does nothing, because
    /// the conductor is built lazily when it is needed.
    pub fn new() -> Self {
        Self(Arc::new(OnceCell::new()))
    }

    /// Get the conductor share for this node. This is an async function because
    /// it needs to initialize the conductor if it hasn't been initialized yet.
    pub async fn lock(&self) -> &RwLock<SweetConductor> {
        let mut config = SweetConductorConfig::standard().no_dpki();
        config.keystore = KeystoreConfig::DangerTestKeystore;
        self.0
            .get_or_init(async { RwLock::new(SweetConductor::from_config(config).await) })
            .await
    }
}

/// This implementation exists so that the parent NetworkTopologyNode can itself
/// implement Arbitrary. It creates an empty once cell which will be filled in
/// by `get` and then ultimately needs to have the parent node apply its state.
impl<'a> Arbitrary<'a> for NetworkTopologyConductor {
    fn arbitrary(_u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(NetworkTopologyConductor::new())
    }
}

/// A graph representing a network topology. Nodes are conductors, edges are
/// connections between conductors, with a list of known agents as cell ids for
/// each edge. This graph is directed, the origin of the edge knows about the
/// target of the edge, but not vice versa.
#[derive(Clone, Debug, Shrinkwrap, Default)]
pub struct NetworkTopology {
    /// The DnaHashes that are in this graph. Used to build cell IDs and install
    /// DNAs in real conductor networks.
    dnas: Vec<DnaFile>,
    /// The graph itself. We DO NOT give mutable access to this graph, because
    /// we need to carefully control how it is mutated. For example we need to
    /// make sure that we don't create self edges or duplicate edges, and we
    /// need to make sure that we don't create edges that have an origin node
    /// referencing a different conductor.
    #[shrinkwrap(main_field)]
    graph: Graph<NetworkTopologyNode, NetworkTopologyEdge, Directed, usize>,
}

/// Errors that can occur when manipulating a `NetworkTopology`.
#[derive(derive_more::Error, derive_more::Display, Debug, PartialEq)]
pub enum NetworkTopologyError {
    /// Failed to remove an edge from the graph.
    #[display(fmt = "Failed to remove an edge from the graph.")]
    EdgeRemove,
    /// Failed to calculate the smallest partition.
    #[display(fmt = "Failed to calculate the smallest partition.")]
    UnknownSmallestPartition,
    /// The smallest partition is empty (no nodes).
    #[display(fmt = "The smallest partition is empty (no nodes).")]
    EmptySmallestPartition,
    /// The node index is not in the graph.
    #[display(fmt = "The node index is not in the graph.")]
    DanglingNodeIndex,
    /// The edge is a self edge.
    #[display(fmt = "The edge is a self edge.")]
    IntegritySelfEdge,
    /// The edge is a duplicate edge.
    #[display(fmt = "The edge is a duplicate edge.")]
    IntegrityDuplicateEdge,
    /// The edge source is not in the graph.
    #[display(fmt = "The edge source is not in the graph.")]
    IntegrityDanglingEdgeSource,
    /// The edge target is not in the graph.
    #[display(fmt = "The edge target is not in the graph.")]
    IntegrityDanglingEdgeTarget,
    /// The edge source conductor does not match the edge source.
    #[display(fmt = "The edge source conductor does not match the edge source.")]
    IntegritySourceConductorMismatch,
    /// The edge target conductor does not match the edge target.
    #[display(fmt = "The edge target conductor does not match the edge target.")]
    IntegrityTargetConductorMismatch,
}

impl From<NetworkTopologyError> for MutationError {
    fn from(e: NetworkTopologyError) -> Self {
        MutationError::User(e.to_string())
    }
}

impl NetworkTopology {
    /// Check the integrity of the graph according to all assumptions that we
    /// make about the graph. This should never fail, and if it does, it's a
    /// bug.
    /// - There are no self edges.
    /// - There are no duplicate edges.
    /// - There are no edges that reference nodes that are not in the graph.
    /// - There are no edges that reference conductors that are not equal to
    ///   their origin node.
    pub fn integrity_check(&self) -> Result<(), NetworkTopologyError> {
        let mut edge_set = HashSet::new();
        for edge in self.edge_references() {
            // Check that there are no self edges.
            if edge.source() == edge.target() {
                return Err(NetworkTopologyError::IntegritySelfEdge);
            }

            // Check that there are no duplicate edges.
            if !edge_set.insert(edge.weight()) {
                return Err(NetworkTopologyError::IntegrityDuplicateEdge);
            }

            {
                // Check that there are no edges that reference nodes that are not in
                // the graph as a source.
                if let Some(source) = self.node_weight(edge.source()) {
                    // Check the source conductor is the same as the edge source
                    // conductor.
                    if edge.weight().source_conductor() != source.conductor() {
                        return Err(NetworkTopologyError::IntegritySourceConductorMismatch);
                    }
                }
                // I think petgraph forbids this, but just in case.
                else {
                    return Err(NetworkTopologyError::IntegrityDanglingEdgeSource);
                }

                // Check that there are no edges that reference nodes that are not in
                // the graph as a target.
                if let Some(target) = self.node_weight(edge.target()) {
                    // Check the target conductor is the same as the edge target
                    // conductor.
                    if edge.weight().target_conductor() != target.conductor() {
                        return Err(NetworkTopologyError::IntegrityTargetConductorMismatch);
                    }
                }
                // I think petgraph forbids this, but just in case.
                else {
                    return Err(NetworkTopologyError::IntegrityDanglingEdgeTarget);
                }
            }
        }

        Ok(())
    }

    /// Apply the state of the network to all its nodes and edges. This is done
    /// by first applying the state of the nodes, then applying the state of the
    /// edges. This is done in two passes because the edges may reference nodes
    /// that have not yet been applied and the application of state may be
    /// sensitive to this.
    pub async fn apply(&mut self) -> anyhow::Result<()> {
        self.integrity_check()?;

        // Push all self DnaFiles into every node.
        for node in self.graph.node_weights_mut() {
            node.ensure_dnas(self.dnas.clone());
        }

        // Apply the state of the nodes.
        for node in self.graph.node_weights_mut() {
            node.apply().await?;
        }

        // Apply the state of the edges.
        for edge in self.graph.edge_weights_mut() {
            edge.apply().await?;
        }

        Ok(())
    }

    /// Get the DnaFiles that are in this graph.
    pub fn dnas(&self) -> &[DnaFile] {
        &self.dnas
    }

    /// Add dnas to the graph.
    pub fn add_dnas(&mut self, dnas: Vec<DnaFile>) {
        self.dnas.extend(dnas);
    }

    /// Return a node by its index or error. This is useful because commonly we
    /// know the index of a node, because we just retrieved it, but we need to
    /// get the node itself. An error in this case is a bug, because we should
    /// never have a node index that is not in the graph.
    pub fn node_or_err(
        &self,
        node_index: usize,
    ) -> Result<&NetworkTopologyNode, NetworkTopologyError> {
        self.graph
            .node_weight(node_index.into())
            .ok_or(NetworkTopologyError::DanglingNodeIndex)
    }

    /// Returns a random node index from the graph.
    pub fn random_node_index<R: rand::Rng>(
        &self,
        rng: &mut R,
    ) -> Result<usize, NetworkTopologyError> {
        let max_node_index = self.node_count() - 1;
        Ok(rng.gen_range(0..=max_node_index))
    }

    /// Returns a random node from the graph.
    pub fn random_node<R: rand::Rng>(
        &self,
        rng: &mut R,
    ) -> Result<&NetworkTopologyNode, NetworkTopologyError> {
        self.node_or_err(self.random_node_index(rng)?)
    }

    /// Private function to build a `UnionFind` from the entire graph with
    /// vertex sets of all nodes and edges.
    fn vertex_sets(&self) -> UnionFind<usize> {
        // Taken from `connected_components` in petgraph.
        let mut vertex_sets = UnionFind::new((**self).node_bound());
        for edge in self.edge_references() {
            let (a, b) = (edge.source(), edge.target());
            // union the two vertices of the edge.
            vertex_sets.union(self.to_index(a), self.to_index(b));
        }
        vertex_sets
    }

    /// Get the number of partitions in the graph. A strict partition consists
    /// of a set of nodes that are all connected to each other, but not to any
    /// nodes outside the partition using a _weakly connected_ path. A weakly
    /// connected path is a path that can be made by removing the directionality
    /// of the edges in the path. So, for example, if we have a graph with three
    /// nodes, A, B, and C, and edges A->B and B->C, then A, B, and C are all
    /// in the same partition, even though there is no path from C to A when
    /// considering the directionality of the edges.
    ///
    /// Holochain networks are expected to be able to heal strict partitions into
    /// much more strongly connected graphs, so we want to make sure that we
    /// generate networks that have strict partitions for testing. This healing
    /// is done by gossiping agent info, and each agent immediately pushes its
    /// agent info upon establishing a connection to another agent, so
    /// theoretically, if we have a network with a strict partition, then the
    /// agents in that partition will all have each other's agent info, and
    /// therefore will be able to connect to each other. In practice, this
    /// may be too slow to be useful, or may not work at all in edge cases,
    /// that's why we want to test it.
    pub fn strict_partitions(&self) -> usize {
        connected_components(self.as_ref())
    }

    /// Remove a random edge from the graph. Returns the index of the edge that
    /// was removed, or an error if no edge was removed.
    pub fn remove_random_edge<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<usize, NetworkTopologyError> {
        let edge_indices = self.edge_indices().collect::<Vec<_>>();
        let max_edge_index = self.edge_count() - 1;
        let index = edge_indices
            .get(rng.gen_range(0..=max_edge_index))
            .ok_or(NetworkTopologyError::EdgeRemove)?
            .index();
        if self.remove_edge_index(index) {
            Ok(index)
        } else {
            Err(NetworkTopologyError::EdgeRemove)
        }
    }

    /// Heal two strict partitions randomly by adding an edge between them.
    /// The added edge has a full view on the target node.
    pub fn heal_random_strict_partition<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), NetworkTopologyError> {
        let vertex_sets = self.vertex_sets();
        let node_index = self.random_node_index(rng)?;
        let mut other_node_indexes = self
            .node_indices()
            .map(|idx| idx.index())
            .collect::<Vec<_>>();
        other_node_indexes.shuffle(rng);

        for other_node_index in other_node_indexes {
            if vertex_sets.find(node_index) != vertex_sets.find(other_node_index) {
                let edge = NetworkTopologyEdge::new_full_view_on_node(
                    self.node_or_err(node_index)?,
                    self.node_or_err(other_node_index)?,
                );
                self.add_simple_edge(node_index, other_node_index, edge);
                break;
            }
        }

        Ok(())
    }

    /// Reassign a random node to the smallest strict partition in the graph.
    /// If the node is already in the smallest strict partition, then do nothing.
    /// Preserves the simplicity of the graph, won't create self edges or
    /// duplicate edges.
    pub fn reassign_random_node_to_smallest_strict_partition<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), NetworkTopologyError> {
        let mut did_reassign = false;
        while !did_reassign {
            let node_index = self.random_node_index(rng)?;
            let labels = self.vertex_sets().into_labeling();

            let mut m: HashMap<usize, usize> = HashMap::new();
            for label in &labels {
                *m.entry(*label).or_default() += 1;
            }
            let representative_of_smallest_partition = m
                .into_iter()
                .min_by_key(|(_, v)| *v)
                .map(|(k, _)| k)
                .ok_or(NetworkTopologyError::UnknownSmallestPartition)?;
            let mut nodes_in_smallest_partition = labels
                .iter()
                .enumerate()
                .filter(|(_i, label)| **label == representative_of_smallest_partition)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            nodes_in_smallest_partition.shuffle(rng);

            let other_node_index = *nodes_in_smallest_partition
                .first()
                .ok_or(NetworkTopologyError::EmptySmallestPartition)?;

            // If the node in the smallest partition is the node we picked,
            // do nothing this round.
            if node_index != other_node_index {
                while let Some(edge) = self.first_edge(node_index.into(), Direction::Outgoing) {
                    self.remove_edge_index(edge.index());
                }
                while let Some(edge) = self.first_edge(node_index.into(), Direction::Incoming) {
                    self.remove_edge_index(edge.index());
                }
                let edge = NetworkTopologyEdge::new_full_view_on_node(
                    self.node_or_err(node_index)?,
                    self.node_or_err(other_node_index)?,
                );

                did_reassign = self.add_simple_edge(node_index, other_node_index, edge);
            }
        }

        Ok(())
    }

    /// Add a node to the graph. Idempotent. Returns true if the node was added,
    /// false if it was not.
    pub fn add_node(&mut self, node: NetworkTopologyNode) -> bool {
        if self.node_weights().any(|n| *n == node) {
            false
        } else {
            self.graph.add_node(node);
            true
        }
    }

    /// Remove a node from the graph by index. Idempotent. Returns true if the
    /// node was removed, false if it was not.
    pub fn remove_node_index(&mut self, node_index: usize) -> bool {
        self.graph.remove_node(node_index.into()).is_some()
    }

    /// Remove an edge from the graph by index. Idempotent. Returns true if the
    /// edge was removed, false if it was not.
    pub fn remove_edge_index(&mut self, edge_index: usize) -> bool {
        self.graph.remove_edge(edge_index.into()).is_some()
    }

    /// Add a simple edge to the graph. A simple edge is an edge that does not
    /// already exist in the graph and does not create a self edge. If the edge
    /// already exists or would create a self edge, or the indexes don't match
    /// the conductors, then do nothing.
    /// Returns true if the edge was added, false if it was not.
    pub fn add_simple_edge(
        &mut self,
        origin: usize,
        target: usize,
        edge: NetworkTopologyEdge,
    ) -> bool {
        let origin_weight = self.node_weight(origin.into());
        if origin_weight.is_none() || origin_weight.unwrap().conductor() != edge.source_conductor()
        {
            return false;
        }
        let target_weight = self.node_weight(target.into());
        if target_weight.is_none() || target_weight.unwrap().conductor() != edge.target_conductor()
        {
            return false;
        }
        if !self.contains_edge(origin.into(), target.into()) && origin != target {
            // Directly mutate the inner graph here.
            self.graph.add_edge(origin.into(), target.into(), edge);
            true
        } else {
            false
        }
    }

    /// Add a random simple edge to the graph. A simple edge is an edge that does
    /// not already exist in the graph and does not create a self edge. If the
    /// edge already exists or would create a self edge, then do nothing.
    pub fn add_random_simple_edge<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<bool, NetworkTopologyError> {
        let a = self.random_node_index(rng)?;
        let b = self.random_node_index(rng)?;

        let edge =
            NetworkTopologyEdge::new_full_view_on_node(self.node_or_err(a)?, self.node_or_err(b)?);

        Ok(self.add_simple_edge(a, b, edge))
    }
}

/// Implement Arbitrary for a Network Topology as an empty network. Use facts
/// or similar to mutate the network into a more interesting state.
/// The network will have some DNA files in it, but no nodes or edges.
impl<'a> Arbitrary<'a> for NetworkTopology {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let graph = Graph::<_, _, _, _>::default();
        let dnas: Result<Vec<DnaFile>, _> = u.arbitrary_iter::<DnaFile>()?.collect();

        Ok(Self { dnas: dnas?, graph })
    }
}

impl PartialEq for NetworkTopology {
    fn eq(&self, other: &Self) -> bool {
        // This is a bit of a hack, but hopefully it works.
        format!(
            "{:?}",
            Dot::with_config(
                self.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        ) == format!(
            "{:?}",
            Dot::with_config(
                other.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        )
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use holochain_zome_types::entropy::unstructured_noise;

    /// Test the arbitrary implementation for NetworkTopology.
    #[test]
    fn test_sweet_topos_arbitrary() -> anyhow::Result<()> {
        let mut u = unstructured_noise();
        let graph = NetworkTopology::arbitrary(&mut u)?;
        // It's arbitrary, so we can't really assert anything about it, but we
        // can print it out to see what it looks like.
        tracing::info!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
        Ok(())
    }

    #[test]
    fn test_network_topology_integrity_check_pass() {
        crate::big_stack_test!(
            async move {
                let mut topology = NetworkTopology::default();

                assert_eq!(Ok(()), topology.integrity_check());

                let node_a = NetworkTopologyNode::new();
                assert!(topology.add_node(node_a.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_b = NetworkTopologyNode::new();
                assert!(topology.add_node(node_b.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let edge = NetworkTopologyEdge::new_full_view_on_node(&node_a, &node_b);

                assert!(topology.add_simple_edge(0, 1, edge.clone()));

                assert_eq!(Ok(()), topology.integrity_check());
            },
            7_000_000
        );
    }

    #[test]
    fn test_network_topology_integrity_check_self_edge_fail() {
        crate::big_stack_test!(
            async move {
                let mut topology = NetworkTopology::default();

                assert_eq!(Ok(()), topology.integrity_check());

                let node = NetworkTopologyNode::new();
                assert!(topology.add_node(node.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let edge = NetworkTopologyEdge::new_full_view_on_node(&node, &node);
                // Adding a self node should fail for a simple edge.
                assert!(!topology.add_simple_edge(0, 0, edge.clone()));
                // Force add it for the test.
                topology.graph.add_edge(0.into(), 0.into(), edge);

                assert_eq!(
                    Err(NetworkTopologyError::IntegritySelfEdge),
                    topology.integrity_check()
                );
            },
            7_000_000
        );
    }

    #[test]
    fn test_network_topology_integrity_check_duplicate_edge_fail() {
        crate::big_stack_test!(
            async move {
                let mut topology = NetworkTopology::default();

                assert_eq!(Ok(()), topology.integrity_check());

                let node_a = NetworkTopologyNode::new();
                assert!(topology.add_node(node_a.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_b = NetworkTopologyNode::new();
                assert!(topology.add_node(node_b.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let edge = NetworkTopologyEdge::new_full_view_on_node(&node_a, &node_b);

                assert!(topology.add_simple_edge(0, 1, edge.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                // Adding the same edge again should fail.
                assert!(!topology.add_simple_edge(0, 1, edge.clone()));
                // Force add it for the test.
                topology.graph.add_edge(0.into(), 1.into(), edge);

                assert_eq!(
                    Err(NetworkTopologyError::IntegrityDuplicateEdge),
                    topology.integrity_check()
                );
            },
            7_000_000
        );
    }

    #[test]
    fn test_network_topology_integrity_check_source_conductor_mismatch_fail() {
        crate::big_stack_test!(
            async move {
                let mut topology = NetworkTopology::default();

                assert_eq!(Ok(()), topology.integrity_check());

                let node_a = NetworkTopologyNode::new();
                assert!(topology.add_node(node_a.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_b = NetworkTopologyNode::new();
                assert!(topology.add_node(node_b.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_c = NetworkTopologyNode::new();
                assert!(topology.add_node(node_c.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let edge = NetworkTopologyEdge::new_full_view_on_node(&node_c, &node_b);

                // Adding the edge should fail.
                assert!(!topology.add_simple_edge(0, 1, edge.clone()));

                // Force add it for the test.
                topology.graph.add_edge(0.into(), 1.into(), edge);

                assert_eq!(
                    Err(NetworkTopologyError::IntegritySourceConductorMismatch),
                    topology.integrity_check()
                );
            },
            7_000_000
        );
    }

    #[test]
    fn test_network_topology_integrity_check_target_conductor_mismatch_fail() {
        crate::big_stack_test!(
            async move {
                let mut topology = NetworkTopology::default();

                assert_eq!(Ok(()), topology.integrity_check());

                let node_a = NetworkTopologyNode::new();
                assert!(topology.add_node(node_a.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_b = NetworkTopologyNode::new();
                assert!(topology.add_node(node_b.clone()));

                assert_eq!(Ok(()), topology.integrity_check());

                let node_c = NetworkTopologyNode::new();
                assert!(topology.add_node(node_c.clone()));

                let edge = NetworkTopologyEdge::new_full_view_on_node(&node_a, &node_c);

                // Adding the edge should fail.
                assert!(!topology.add_simple_edge(0, 1, edge.clone()));

                // Force add it for the test.
                topology.graph.add_edge(0.into(), 1.into(), edge);

                assert_eq!(
                    Err(NetworkTopologyError::IntegrityTargetConductorMismatch),
                    topology.integrity_check()
                );
            },
            7_000_000
        );
    }
}
