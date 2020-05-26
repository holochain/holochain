/// Error type for Holochain P2p.
#[derive(Debug, thiserror::Error)]
pub enum HolochainP2pError {
    /// GhostError
    #[error(transparent)]
    GhostError(#[from] ghost_actor::GhostError),

    /// KitsuneP2pError
    #[error(transparent)]
    KitsuneP2pError(#[from] kitsune_p2p::KitsuneP2pError),
}

pub mod actor;
pub mod event;
