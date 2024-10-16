use crate::sweettest::sweet_topos::network::NetworkTopology;
use contrafact::Fact;
use contrafact::Generator;
use contrafact::Mutation;

/// Fact:
/// - The network has a specific density according to graph theory.
///
/// This is the number of edges divided by the maximum number of edges.
/// This is a number between 0 and 1. 0 means no edges. 1 means every node
/// is connected to every other node.
/// This is a directed graph, so the maximum number of edges is n * (n - 1).
/// This measurement only makes sense for simple graphs, so we assume that.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseNetworkFact {
    /// The density of the network. Number of edges divided by the maximum
    /// number of edges. Only makes sense for simple graphs.
    pub density: f64,
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
    pub fn max_edge_count(graph: &NetworkTopology) -> usize {
        let node_count = graph.node_count();
        node_count * (node_count - 1)
    }

    /// This is the number of edges that we want to have in the graph. It is
    /// a truncation of the density times the maximum number of edges, so we're
    /// rounding down.
    pub fn target_edge_count(&self, graph: &NetworkTopology) -> usize {
        (self.density * Self::max_edge_count(graph) as f64) as usize
    }
}

impl<'a> Fact<'a, NetworkTopology> for DenseNetworkFact {
    fn mutate(
        &mut self,
        g: &mut Generator<'a>,
        mut graph: NetworkTopology,
    ) -> Mutation<NetworkTopology> {
        let target_edge_count = self.target_edge_count(&graph);
        let mut rng = super::rng_from_generator(g);

        // Add edges until we reach the desired density.
        while graph.edge_count() < target_edge_count {
            graph.add_random_simple_edge(&mut rng)?;
        }

        // Remove edges until we reach the desired density.
        while graph.edge_count() > target_edge_count {
            graph.remove_random_edge(&mut rng)?;
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
    use super::DenseNetworkFact;
    use crate::prelude::unstructured_noise;
    use crate::sweettest::fact::partition::StrictlyPartitionedNetworkFact;
    use crate::sweettest::fact::size::SizedNetworkFact;
    use crate::sweettest::sweet_topos::network::NetworkTopology;
    use contrafact::Fact;
    use petgraph::dot::{Config, Dot};

    /// Test that we can build a dense network fact with `DenseNetworkFact::new`.
    #[test]
    fn test_dense_network_fact_new() {
        let a = DenseNetworkFact::new(0.5);
        let b = DenseNetworkFact { density: 0.5 };
        assert_eq!(a, b);
    }

    #[test]
    fn test_sweet_topos_dense_network() {
        crate::big_stack_test!(async move {
            let mut g = unstructured_noise().into();
            let mut size_fact = SizedNetworkFact {
                nodes: 12,
                agents: 1..=2,
            };
            let mut density_fact = DenseNetworkFact { density: 0.3 };
            let mut graph = NetworkTopology::default();
            graph = size_fact.mutate(&mut g, graph).unwrap();
            tracing::info!(
                "{:?}",
                Dot::with_config(graph.as_ref(), &[Config::NodeNoLabel, Config::EdgeNoLabel,],)
            );
            graph = density_fact.mutate(&mut g, graph).unwrap();
            tracing::info!(
                "{:?}",
                Dot::with_config(graph.as_ref(), &[Config::NodeNoLabel, Config::EdgeNoLabel,],)
            );
            let mut partition_fact = StrictlyPartitionedNetworkFact {
                partitions: 1,
                efficiency: 1.0,
            };
            graph = partition_fact.mutate(&mut g, graph).unwrap();
            tracing::info!(
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
        });
    }
}
