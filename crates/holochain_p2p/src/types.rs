/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
pub enum HolochainP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// KitsuneP2pError
    #[error(transparent)]
    KitsuneP2pError(#[from] kitsune_p2p::KitsuneP2pError),

    /// Custom
    #[error("Custom: {0}")]
    Custom(Box<dyn std::error::Error + Send + Sync>),
}

impl HolochainP2pError {
    /// promote a custom error type to a TransportError
    pub fn custom(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Custom(e.into())
    }
}

pub mod actor;
pub mod event;
