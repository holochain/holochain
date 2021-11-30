mod peer_view_alpha;
pub use peer_view_alpha::*;

use crate::DhtArc;

pub mod gaps;

/// A Strategy for generating PeerViews.
/// The enum allows us to add new strategies over time.
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

/// A "view" of the peers in a neighborhood. The view consists of a few
/// observations about the distribution of peers within a particular arc, used
/// to make inferences about the rest of the (out-of-view) DHT, ultimately
/// enabling the calculation of the target arc size for the agent who has this View.
///
/// The enum allows us to add different views (and different calculations of
/// target arc length) over time.
#[derive(Debug, Clone, derive_more::From)]
pub enum PeerView {
    Alpha(PeerViewAlpha),
}

impl PeerView {
    /// Given the current view of a peer and the peer's current coverage,
    /// this returns the next step to take in reaching the ideal coverage.
    pub fn next_coverage(&self, current: f64) -> f64 {
        match self {
            Self::Alpha(s) => s.next_coverage(current),
        }
    }
}
