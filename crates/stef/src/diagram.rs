use std::collections::{HashMap, HashSet};

use crate::State;
use petgraph::graph::DiGraph;
use proptest::prelude::Arbitrary;

pub trait StateDiagrammable: State + Clone + Into<Self::Node>
where
    Self::Action: Clone + Arbitrary,
{
    type Node: Clone + Eq + ToString + std::hash::Hash;
    type Edge: Clone + Eq + std::hash::Hash + From<Self::Action>;

    /// Generate a "Monte Carlo state diagram" of this state machine.
    fn state_diagram(self, walks: u32, walk_len: u32) -> DiGraph<String, Self::Edge> {
        let mut graph = DiGraph::new();
        let mut node_indices = HashMap::new();
        let mut edges = HashSet::new();

        let initial: Self::Node = self.clone().into();
        let ix = graph.add_node(initial.to_string());
        node_indices.insert(initial, ix);

        for _ in 0..walks {
            let mut prev = ix;
            for (action, state) in take_a_walk(self.clone(), walk_len) {
                let edge = Self::Edge::from(action);
                let node = state.into();
                let ix = if let Some(ix) = node_indices.get(&node) {
                    *ix
                } else {
                    let ix = graph.add_node(node.to_string());
                    node_indices.insert(node, ix);
                    ix
                };
                if edges.insert((prev, ix, edge.clone())) {
                    graph.add_edge(prev, ix, edge);
                }
                prev = ix;
            }
        }

        graph
    }
}

fn take_a_walk<S: StateDiagrammable>(mut s: S, len: u32) -> Vec<(S::Action, S)>
where
    <S as State>::Action: Arbitrary + Clone,
{
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::default();
    let mut steps = vec![];
    for _ in 0..len {
        let action: S::Action = S::Action::arbitrary()
            .new_tree(&mut runner)
            .unwrap()
            .current();
        s.transition(action.clone());
        steps.push((action, s.clone()));
    }
    steps
}
