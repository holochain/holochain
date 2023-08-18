use stef::State;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GossipRoundError {
    #[error("busy")]
    Busy,

    #[error("Error decoding gossip message: {0}")]
    DecodeError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GossipError {
    #[error(transparent)]
    GossipRoundError(#[from] GossipRoundError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),

    #[error("downstream error (TODO remove)")]
    Downstream(Box<dyn std::error::Error + Send + Sync>),
}

impl GossipError {
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into())
    }

    pub fn downstream(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Downstream(e.into())
    }
}

pub type GossipRoundResult<T> = Result<T, GossipRoundError>;
pub type GossipResult<T> = Result<T, GossipError>;
