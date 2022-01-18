#[derive(Debug)]
pub enum GossipError {
    TopologyMismatch,
    TimesOutOfSync,
    ArqPowerDiffTooLarge,
    ArqSetMismatchForDiff,
}

pub type GossipResult<T> = Result<T, GossipError>;
