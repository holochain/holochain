use crate::*;
use std::sync::Arc;

/// Concise representation of data held by various agents in a sharded scenario,
/// without having to refer to explicit op hashes or locations.
///
/// This type is intended to be used to easily define arbitrary sharded network scenarios,
/// to test various cases of local sync and gossip. It's expected that we'll eventually have a
/// small library of such scenarios, defined in terms of this type.
///
/// See [`generate_ops_for_overlapping_arcs`] for usage detail.
pub struct OwnershipData {
    /// Total number of op hashes to be generated
    pub total_ops: usize,
    /// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
    pub agents: Vec<OwnershipDataAgent>,
}

impl OwnershipData {
    /// Construct `OwnershipData` from a more compact "untagged" format using
    /// tuples instead of structs. This is intended to be the canonical constructor.
    pub fn from_compact(total_ops: usize, v: Vec<OwnershipDataAgentCompact>) -> Self {
        Self {
            total_ops,
            agents: v
                .into_iter()
                .map(|(agent, arc_indices, hash_indices)| OwnershipDataAgent {
                    agent,
                    arc_indices,
                    hash_indices,
                })
                .collect(),
        }
    }
}

/// Declares arcs and ownership in terms of indices into a vec of generated op hashes.
pub struct OwnershipDataAgent {
    /// The agent in question
    pub agent: Arc<KitsuneAgent>,
    /// The start and end indices of the arc for this agent
    pub arc_indices: (usize, usize),
    /// The indices of ops to consider as owned
    pub hash_indices: Vec<usize>,
}

/// Same as [`OwnershipDataAgent`], but using a tuple instead of a struct.
/// It's just more compact.
pub type OwnershipDataAgentCompact = (Arc<KitsuneAgent>, (usize, usize), Vec<usize>);
