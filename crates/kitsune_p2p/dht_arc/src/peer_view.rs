mod peer_view_alpha;
pub use peer_view_alpha::*;
// mod peer_view_beta;
// pub use peer_view_beta::*;

use crate::DhtArc;

pub mod gaps;

#[derive(Debug, Clone, derive_more::From)]
pub enum PeerStrat {
    Alpha(PeerStratAlpha),
}

impl Default for PeerStrat {
    fn default() -> Self {
        PeerStratAlpha::default().into()
    }
}

impl PeerStrat {
    pub fn view(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerView {
        match self {
            Self::Alpha(s) => s.view(arc, peers).into(),
        }
    }

    pub fn view_unchecked(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerView {
        match self {
            Self::Alpha(s) => s.view_unchecked(arc, peers).into(),
        }
    }
}

#[derive(Debug, Clone, derive_more::From)]
pub enum PeerView {
    Alpha(PeerViewAlpha),
}
