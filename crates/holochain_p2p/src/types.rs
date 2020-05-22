/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
pub enum HolochainP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),
}

pub mod actor;
pub mod event;
