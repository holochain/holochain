mod peer_view_alpha;
pub use peer_view_alpha::*;

pub mod gaps;

#[derive(Debug, Clone, derive_more::From)]
pub enum PeerStrat {
    Alpha(PeerStratAlpha),
}

#[derive(Debug, Clone, derive_more::From)]
pub enum PeerView {
    Alpha(PeerViewAlpha),
}
