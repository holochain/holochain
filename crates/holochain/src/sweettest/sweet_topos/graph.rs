use arbitrary::Arbitrary;
use shrinkwraprs::Shrinkwrap;
use derive_more::DerefMut;
use holo_hash::DnaHash;
use petgraph::prelude::*;
use petgraph::dot::{Dot, Config};
use crate::sweettest::sweet_topos::node::NetworkTopologyNode;
use crate::sweettest::sweet_topos::edge::NetworkTopologyEdge;
use petgraph::algo::connected_components;
use arbitrary::Unstructured;

#[derive(Clone, Debug, Shrinkwrap, Default, DerefMut)]
pub struct NetworkTopologyGraph{
    dnas: Vec<DnaHash>,
    #[shrinkwrap(main_field)]
    #[deref_mut]
    graph: Graph<NetworkTopologyNode, NetworkTopologyEdge, Directed, usize>,
}

impl NetworkTopologyGraph {

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
}

/// Implement arbitrary for NetworkTopologyGraph by simply iterating over some
/// arbitrary nodes and edges and adding them to the graph. This allows self
/// edges and duplicate edges, but that's fine for our purposes as it will simply
/// cause agent info to be added multiple times or to the same agent.
impl<'a> Arbitrary<'a> for NetworkTopologyGraph {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut graph = Graph::default();

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

        let dnas: Result<Vec<DnaHash>> = u.arbitrary_iter::<DnaHash>()?.collect();

        Ok(Self{
            dnas: dnas?,
            graph,
        })
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