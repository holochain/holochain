#[derive(Debug, thiserror::Error)]
pub enum GossipError {
    #[error("The fundamental parameters of Op region spacetime are mismatched between nodes.")]
    TopologyMismatch,
    #[error("System times between nodes are too far apart to be able to gossip.")]
    TimesOutOfSync,
    #[error("Attempting to gossip with too large a discrepancy in chunk size")]
    ArqPowerDiffTooLarge,
    #[error("Attempting to gossip with a mismatch in the common arc set")]
    ArqSetMismatchForDiff,
}

pub type GossipResult<T> = Result<T, GossipError>;
