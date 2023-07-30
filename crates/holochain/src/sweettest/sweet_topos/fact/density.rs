// use super::edge::FullAgentViewFact;
use crate::sweettest::sweet_topos::graph::NetworkTopologyGraph;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;
// use contrafact::MutationError;

/// Fact:
/// - The network has a specific density according to graph theory.
/// This is the number of edges divided by the maximum number of edges.
/// This is a number between 0 and 1. 0 means no edges. 1 means every node
/// is connected to every other node.
/// This is a directed graph, so the maximum number of edges is n * (n - 1).
/// This measument only makes sense for simple graphs, so we assume that.
#[derive(Clone, Debug)]
pub struct DenseNetworkFact {
    density: f64,
}

impl DenseNetworkFact {
    /// Create a new fact with the given density.
    pub fn new(density: f64) -> Self {
        Self { density }
    }

    /// This is the maximum number of edges that a graph with the given number
    /// of nodes can have. Assumes that the graph is directed. Assumes that
    /// there are no self edges. Assumes that there are no duplicate edges.
    /// Assumes that the graph is simple.
    /// Petgraph DOES NOT make any of these assumptions, so we have to do this
    /// ourselves.
    pub fn max_edge_count(graph: &NetworkTopologyGraph) -> usize {
        let node_count = graph.node_count();
        node_count * (node_count - 1)
    }

    /// This is the number of edges that we want to have in the graph. It is
    /// a truncation of the density times the maximum number of edges, so we're
    /// rounding down.
    pub fn target_edge_count(&self, graph: &NetworkTopologyGraph) -> usize {
        (self.density * Self::max_edge_count(&graph) as f64) as usize
    }
}

impl<'a> Fact<'a, NetworkTopologyGraph> for DenseNetworkFact {
    fn mutate(
        &mut self,
        g: &mut Generator<'a>,
        mut graph: NetworkTopologyGraph,
    ) -> Mutation<NetworkTopologyGraph> {
        let target_edge_count = self.target_edge_count(&graph);
        let mut rng: _ = super::rng_from_generator(g);

        // Add edges until we reach the desired density.
        while graph.edge_count() < target_edge_count {
            graph.add_random_simple_edge(&mut rng)?;
            // let a = graph.random_node(g)?;
            // // let max_node_index = graph.node_count() - 1;
            // let b = graph.random_node(g)?;
            // // let a = g
            // //     .int_in_range(0..=max_node_index, "could not select a node")?
            // //     .into();
            // // let b = g
            // //     .int_in_range(0..=max_node_index, "could not select a node")?
            // //     .into();

            // // Don't add an edge if it already exists or if it's a self edge.
            // // Density calculations assume this so we can't introduce any.
            // if !graph.contains_edge(a, b) && a != b {
            //     let edge = FullAgentViewFact {
            //         target: graph.node_or_err(b)?.target().clone(),
            //     }
            //     .build_fallible(g)
            //     .map_err(|_| MutationError::Exception("Failed to build agent view".into()))?;
            //     graph.add_edge(a, b, edge);
            // }
        }

        // Remove edges until we reach the desired density.
        while graph.edge_count() > target_edge_count {
            graph.remove_random_edge(&mut rng)?;
            // let edge_indices = graph.edge_indices().collect::<Vec<_>>();
            // let max_edge_index = graph.edge_count() - 1;
            // graph.remove_edge(
            //     edge_indices
            //         .iter()
            //         .nth(
            //             g.int_in_range(0..=max_edge_index, "could not select an edge to remove")?
            //                 .into(),
            //         )
            //         .ok_or(MutationError::Exception(
            //             "could not select an edge to remove".to_string(),
            //         ))?
            //         .clone(),
            // );
        }

        Ok(graph)
    }

    fn label(&self) -> String {
        format!("DenseNetworkFact {{ density: {} }}", self.density)
    }

    fn labeled(self, _: impl ToString) -> Self {
        todo!()
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    /// Test that we can build a dense network fact with `DenseNetworkFact::new`.
    #[test]
    fn test_dense_network_fact_new() {
        let a = DenseNetworkFact::new(0.5);
        let b = DenseNetworkFact { density: 0.5 };
        assert_eq!(a, b);
    }

    #[test]
    fn test_sweet_topos_dense_network() {
        let mut g = unstructured_noise().into();
        let size_fact = SizedNetworkFact { nodes: 12 };
        let density_fact = DenseNetworkFact { density: 0.3 };
        let mut graph = NetworkTopologyGraph::default();
        graph = size_fact.mutate(graph, &mut g).unwrap();
        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    //     Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
        graph = density_fact.mutate(graph, &mut g).unwrap();
        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    //     Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
        let partition_fact = StrictlyPartitionedNetworkFact {
            partitions: 1,
            efficiency: 1.0,
        };
        graph = partition_fact.mutate(graph, &mut g).unwrap();
        println!(
            "{:?}",
            Dot::with_config(
                graph.as_ref(),
                &[
                    //     Config::GraphContentOnly,
                    Config::NodeNoLabel,
                    Config::EdgeNoLabel,
                ],
            )
        );
    }
}
