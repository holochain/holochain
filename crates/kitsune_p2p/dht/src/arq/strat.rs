use kitsune_p2p_dht_arc::{DhtArc, PeerStratAlpha, PeerStratBeta};

use crate::quantum::Topology;

use super::{Arq, PeerView, PeerViewQ};

/// A Strategy for generating PeerViews.
/// The enum allows us to add new strategies over time.
#[derive(Debug, Clone, derive_more::From)]
pub enum PeerStrat {
    Alpha(PeerStratAlpha),
    Beta(PeerStratBeta),
    Quantized(ArqStrat),
}

impl Default for PeerStrat {
    fn default() -> Self {
        ArqStrat::default().into()
    }
}

impl PeerStrat {
    pub fn view(&self, topo: Topology, arc: DhtArc, peers: &[DhtArc]) -> PeerView {
        match self {
            Self::Alpha(s) => s.view(arc, peers).into(),
            Self::Beta(s) => s.view(arc, peers).into(),
            Self::Quantized(s) => {
                let peers = peers
                    .iter()
                    .map(|p| Arq::from_dht_arc_approximate(&topo, s, p))
                    .collect();
                PeerViewQ::new(topo, s.clone(), peers).into()
            }
        }
    }

    pub fn view_unchecked(&self, topo: Topology, arc: DhtArc, peers: &[DhtArc]) -> PeerView {
        match self {
            Self::Alpha(s) => s.view_unchecked(arc, peers).into(),
            Self::Beta(s) => s.view_unchecked(arc, peers).into(),
            Self::Quantized(s) => {
                // TODO: differentiate checked vs unchecked
                let peers = peers
                    .iter()
                    .map(|p| Arq::from_dht_arc_approximate(&topo, s, p))
                    .collect();
                PeerViewQ::new(topo, s.clone(), peers).into()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArqStrat {
    /// The minimum coverage the DHT seeks to maintain.
    ///
    /// This is the whole purpose for arc resizing.
    pub min_coverage: f64,

    /// A multiplicative factor of the min coverage which defines a max.
    /// coverage. We want coverage to be between the min and max coverage.
    /// This is expressed in terms of a value > 0 and < 1. For instance,
    /// a min coverage of 50 with a buffer of 0.2 implies a max coverage of 60.
    pub buffer: f64,

    /// If the difference between the arq's power and the median power of all
    /// peer arqs (including this one) is greater than this diff,
    /// then don't requantize:
    /// just keep growing or shrinking past the min/max chunks value.
    ///
    /// This parameter determines how likely it is for there to be a difference in
    /// chunk sizes between two agents' arqs. It establishes the tradeoff between
    /// the size of payloads that must be sent and the extra depth of Fenwick
    /// tree data that must be stored (to accomodate agents whose power is
    /// lower than ours).
    ///
    /// This parameter is also what allows an arq to shrink to zero in a
    /// reasonable number of steps. Without this limit on power diff, we would
    /// keep requantizing until the power was 0 before shrinking to the empty arc.
    /// We may shrink to zero if our neighborhood is significantly over-covered,
    /// which can happen if a number of peers decide to keep their coverage
    /// higher than the ideal equilibrium value.
    ///
    /// Note that this parameter does not guarantee that any agent's arq
    /// will have a power +/- this diff from our power, but we may decide to
    /// choose not to gossip with agents whose power falls outside the range
    /// defined by this diff. TODO: do this.
    pub max_power_diff: u8,

    /// If at any time the number of peers seen by a node is less than the
    /// extrapolated coverage scaled by this factor, then we assume that we need
    /// to grow our arc so that we can see more peers.
    /// In other words, we are "slacking" if at any time:
    ///     num_peers < extrapolated_coverage * slack_factor
    ///
    /// If this is set too high, it may prevent arcs from legitimately shrinking.
    /// If set too low, it will hamper the ability for extremely small arcs to
    /// reach a proper size
    pub slacker_ratio: f64,

    /// If the standard deviation of the powers of each arq in this view is
    /// greater than this threshold, then we might do something different when
    /// it comes to our decision to requantize. For now, just print a warning.
    ///
    /// TODO: this can probably be expressed in terms of `max_power_diff`.
    pub power_std_dev_threshold: f64,
}

impl Default for ArqStrat {
    fn default() -> Self {
        Self {
            min_coverage: 50.0,
            // this buffer implies min-max chunk count of 8-16
            buffer: 0.143,
            power_std_dev_threshold: 1.0,
            max_power_diff: 2,
            slacker_ratio: 0.75,
        }
    }
}

impl ArqStrat {
    /// The midline between min and max coverage
    pub fn midline_coverage(&self) -> f64 {
        (self.min_coverage + self.max_coverage()) / 2.0
    }

    /// The max coverage as expressed by the min coverage and the buffer
    pub fn max_coverage(&self) -> f64 {
        (self.min_coverage * (self.buffer + 1.0)).ceil()
    }

    /// The width of the buffer range
    pub fn buffer_width(&self) -> f64 {
        self.min_coverage * self.buffer
    }

    /// The lower bound of number of chunks to maintain in an arq.
    /// When the chunk count falls below this number, halve the chunk size.
    pub fn min_chunks(&self) -> u32 {
        self.chunk_count_threshold().ceil() as u32
    }

    /// The upper bound of number of chunks to maintain in an arq.
    /// When the chunk count exceeds this number, double the chunk size.
    ///
    /// This is expressed in terms of min_chunks because we want this value
    /// to always be odd -- this is because when growing the arq, we need to
    /// downshift the power, and we can only downshift losslessly if the count
    /// is even, and the most common case of exceeding the max_chunks is
    /// is to exceed the max_chunks by 1, which would be an even number.
    pub fn max_chunks(&self) -> u32 {
        let max_chunks = self.min_chunks() * 2 - 1;
        assert!(max_chunks % 2 == 1);
        max_chunks
    }

    /// The floor of the log2 of the max_chunks.
    /// For the default of 15, floor(log2(15)) = 3
    pub fn max_chunks_log2(&self) -> u8 {
        (self.max_chunks() as f64).log2().floor() as u8
    }

    /// The chunk count which gives us the quantization resolution appropriate
    /// for maintaining the buffer when adding/removing single chunks.
    /// Used in `min_chunks` and `max_chunks`.
    ///
    /// See this doc for rationale:
    /// https://hackmd.io/@hololtd/r1IAIbr5Y/https%3A%2F%2Fhackmd.io%2FK_fkBj6XQO2rCUZRRL9n2g
    fn chunk_count_threshold(&self) -> f64 {
        (self.buffer + 1.0) / self.buffer
    }

    pub fn summary(&self) -> String {
        format!(
            "
        min coverage: {}
        max coverage: {}
        min chunks:   {}
        max chunks:   {}
        ",
            self.min_coverage,
            self.max_coverage(),
            self.min_chunks(),
            self.max_chunks()
        )
    }
}
