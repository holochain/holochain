use crate::sweettest::sweet_topos::edge::NetworkTopologyEdge;
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use contrafact::MutationError;
use holo_hash::DnaHash;
use petgraph::algo::connected_components;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use petgraph::unionfind::UnionFind;
use petgraph::visit::NodeIndexable;
use rand::seq::SliceRandom;
use shrinkwraprs::Shrinkwrap;
use std::collections::HashMap;

/// A graph representing a network topology. Nodes are conductors, edges are
/// connections between conductors, with a list of known agents as cell ids for
/// each edge. This graph is directed, the origin of the edge knows about the
/// target of the edge, but not vice versa.
#[derive(Clone, Debug, Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct NetworkTopologyGraph {
    /// The DnaHashes that are in this graph. Used to build cell IDs and install
    /// DNAs in real conductor networks.
    dnas: Vec<DnaHash>,
    /// The graph itself.
    #[shrinkwrap(main_field)]
    pub graph: Graph<NetworkTopologyNode, NetworkTopologyEdge, Directed, usize>,
}

/// Errors that can occur when manipulating a `NetworkTopologyGraph`.
pub enum NetworkTopologyGraphError {
    /// Failed to remove an edge from the graph.
    EdgeRemove,
    /// Failed to calculate the smallest partition.
    UnknownSmallestPartition,
    /// The smallest partition is empty (no nodes).
    EmptySmallestPartition,
    /// The node index is not in the graph.
    DanglingNodeIndex,
}

impl ToString for NetworkTopologyGraphError {
    fn to_string(&self) -> String {
        match self {
            Self::EdgeRemove => "Failed to remove an edge from the graph.".to_string(),
            Self::UnknownSmallestPartition => {
                "Failed to calculate the smallest partition.".to_string()
            }
            Self::EmptySmallestPartition => "The smallest partition is empty.".to_string(),
            Self::DanglingNodeIndex => "The node index is not in the graph.".to_string(),
        }
    }
}

impl From<NetworkTopologyGraphError> for MutationError {
    fn from(e: NetworkTopologyGraphError) -> Self {
        MutationError::Exception(e.to_string())
    }
}

impl NetworkTopologyGraph {
    /// Return a node by its index or error. This is useful because commonly we
    /// know the index of a node, because we just retrieved it, but we need to
    /// get the node itself. An error in this case is a bug, because we should
    /// never have a node index that is not in the graph.
    pub fn node_or_err(
        &self,
        node_index: usize,
    ) -> Result<&NetworkTopologyNode, NetworkTopologyGraphError> {
        self.graph
            .node_weight(node_index.into())
            .ok_or(NetworkTopologyGraphError::DanglingNodeIndex)
    }

    /// Returns a random node index from the graph.
    pub fn random_node_index<R: rand::Rng>(
        &self,
        rng: &mut R,
    ) -> Result<usize, NetworkTopologyGraphError> {
        let max_node_index = self.node_count() - 1;
        Ok(rng.gen_range(0..=max_node_index))
    }

    /// Returns a random node from the graph.
    pub fn random_node<R: rand::Rng>(
        &self,
        rng: &mut R,
    ) -> Result<&NetworkTopologyNode, NetworkTopologyGraphError> {
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

    /// Remove a random edge from the graph.
    pub fn remove_random_edge<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), NetworkTopologyGraphError> {
        let edge_indices = self.edge_indices().collect::<Vec<_>>();
        let max_edge_index = self.edge_count() - 1;
        self.remove_edge(
            edge_indices
                .iter()
                .nth(
                    rng.gen_range(0..=max_edge_index)
                        // .map_err(|_| NetworkTopologyGraphError::EdgeRemove)?
                        .into(),
                )
                .ok_or(NetworkTopologyGraphError::EdgeRemove)?
                .clone(),
        );
        Ok(())
    }

    /// Heal two strict partitions randomly by adding an edge between them.
    /// The added edge has a full view on the target node.
    pub fn heal_random_strict_partition<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), NetworkTopologyGraphError> {
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
                    self.node_weight(other_node_index.into())
                        .ok_or(NetworkTopologyGraphError::DanglingNodeIndex)?,
                );
                // let edge = FullAgentViewFact {
                //     origin: node_index,
                //     target: other_node_index,
                // }
                // .build_fallible(g)?;
                self.add_edge(node_index.into(), other_node_index.into(), edge);
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
    ) -> Result<(), NetworkTopologyGraphError> {
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
            .ok_or(NetworkTopologyGraphError::UnknownSmallestPartition)?;
        let mut nodes_in_smallest_partition = labels
            .iter()
            .enumerate()
            .filter(|(i, label)| **label == representative_of_smallest_partition)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        nodes_in_smallest_partition.shuffle(rng);

        let other_node_index = *nodes_in_smallest_partition
            .iter()
            .next()
            .ok_or(NetworkTopologyGraphError::EmptySmallestPartition)?;

        // If the node in the smallest partition is the node we picked,
        // do nothing this round.
        if node_index != other_node_index {
            while let Some(edge) = self.first_edge(node_index.into(), Direction::Outgoing) {
                self.remove_edge(edge);
            }
            while let Some(edge) = self.first_edge(node_index.into(), Direction::Incoming) {
                self.remove_edge(edge);
            }
            let edge = NetworkTopologyEdge::new_full_view_on_node(
                self.node_weight(other_node_index.into())
                    .ok_or(NetworkTopologyGraphError::DanglingNodeIndex)?,
            );

            // let edge = FullAgentViewFact {
            //     origin: node_index,
            //     target: other_node_index,
            // }
            // .build_fallible(g)?;
            self.add_simple_edge(node_index.into(), other_node_index.into(), edge)?;
        }

        Ok(())
    }

    /// Add a simple edge to the graph. A simple edge is an edge that does not
    /// already exist in the graph and does not create a self edge. If the edge
    /// already exists or would create a self edge, then do nothing.
    pub fn add_simple_edge(
        &mut self,
        origin: usize,
        target: usize,
        edge: NetworkTopologyEdge,
    ) -> Result<(), NetworkTopologyGraphError> {
        if !self.contains_edge(origin.into(), target.into()) && origin != target {
            let edge = NetworkTopologyEdge::default();
            self.add_edge(origin.into(), target.into(), edge);
        }
        Ok(())
    }

    /// Add a random simple edge to the graph. A simple edge is an edge that does
    /// not already exist in the graph and does not create a self edge. If the
    /// edge already exists or would create a self edge, then do nothing.
    pub fn add_random_simple_edge<R: rand::Rng>(
        &mut self,
        rng: &mut R,
    ) -> Result<(), NetworkTopologyGraphError> {
        let a = self.random_node_index(rng)?;
        let b = self.random_node_index(rng)?;

        let edge = NetworkTopologyEdge::new_full_view_on_node(self.node_or_err(b)?);

        self.add_simple_edge(a, b, edge)
    }
}

/// Implement arbitrary for NetworkTopologyGraph by simply iterating over some
/// arbitrary nodes and edges and adding them to the graph. This allows self
/// edges and duplicate edges, but that's fine for our purposes as it will simply
/// cause agent info to be added multiple times or to the same agent.
impl<'a> Arbitrary<'a> for NetworkTopologyGraph {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut graph = Graph::<_, _, _, _>::default();

        // Get an iterator of arbitrary `NetworkTopologyNode`s.
        let nodes = u.arbitrary_iter::<NetworkTopologyNode>()?;
        for node in nodes {
            graph.add_node(node?);
        }

        if graph.node_count() > 0 {
            // Add some edges.
            let edges: arbitrary::Result<Vec<NetworkTopologyEdge>> =
                u.arbitrary_iter::<NetworkTopologyEdge>()?.collect();
            for edge in edges? {
                let max_node_index = graph.node_count() - 1;
                let a = u.int_in_range(0..=max_node_index)?.into();
                let b = u.int_in_range(0..=max_node_index)?.into();
                graph.add_edge(a, b, edge);
            }
        }

        let dnas: Result<Vec<DnaHash>, _> = u.arbitrary_iter::<DnaHash>()?.collect();

        Ok(Self { dnas: dnas?, graph })
    }
}

impl PartialEq for NetworkTopologyGraph {
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

    /// Test the arbitrary implementation for NetworkTopologyGraph.
    #[test]
    fn test_sweet_topos_arbitrary() -> anyhow::Result<()> {
        let mut u = unstructured_noise();
        let graph = NetworkTopologyGraph::arbitrary(&mut u)?;
        // It's arbitrary, so we can't really assert anything about it, but we
        // can print it out to see what it looks like.
        println!(
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
}
