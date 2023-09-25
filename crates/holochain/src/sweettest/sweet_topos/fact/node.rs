use contrafact::Generator;
use contrafact::Mutation;
use std::ops::RangeInclusive;

/// A contrafact fact for generating a network node with a given number of agents.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SizedNodeFact {
    /// The number of agents in the node.
    agents: usize,
}

impl SizedNodeFact {
    /// Create a new fact with the given number of agents.
    pub fn new(agents: usize) -> Self {
        Self { agents }
    }

    /// Get the number of agents in the node.
    pub fn agents(&self) -> usize {
        self.agents
    }

    /// Create a new fact with a number of agents in the given range.
    pub fn from_range(g: &mut Generator, agents: RangeInclusive<usize>) -> Mutation<Self> {
        Ok(Self {
            agents: g.int_in_range(agents, || "Couldn't build a fact in the range.")?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::prelude::unstructured_noise;
    use contrafact::Generator;

    // Test that we can build a sized node fact with `SizedNodeFact::new`.
    #[test]
    fn test_sized_node_fact_new() {
        let a = SizedNodeFact::new(3);
        let b = SizedNodeFact { agents: 3 };
        assert_eq!(a, b);
    }

    // Test that we can build a sized node fact from a range with `SizedNodeFact::from_range`.
    #[test]
    fn test_sized_node_fact_from_range() {
        let mut g = Generator::from(unstructured_noise());
        let a = SizedNodeFact::from_range(&mut g, 1..=3).unwrap();
        assert!(a.agents >= 1 && a.agents <= 3);
    }
}
