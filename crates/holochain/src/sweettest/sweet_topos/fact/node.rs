/// A contrafact fact for generating a network node with a given number of agents.
pub struct SizedNodeFact {
    /// The number of agents in the node.
    /// Ideally this would be a range, but we can't do that yet.
    agents: usize,
}
