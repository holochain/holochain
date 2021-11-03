use super::{DhtArc, MAX_HALF_LENGTH};
use crate::*;

/// When sampling a section of the arc we can
/// collect all the other peer [`DhtArc`]s into a
/// DhtBucket.
/// All the peer arcs arc contained within the buckets filter arc.
/// The filter is this peer's "view" into their section of the dht arc.
pub struct DhtArcBucket {
    /// The arc used to filter this bucket.
    filter: DhtArc,
    /// The arcs in this bucket.
    arcs: Vec<DhtArc>,
}

impl DhtArcBucket {
    /// Select only the arcs that fit into the bucket.
    pub fn new<I: IntoIterator<Item = DhtArc>>(filter: DhtArc, arcs: I) -> Self {
        let arcs = arcs
            .into_iter()
            .filter(|a| filter.contains(a.center_loc))
            .collect();
        Self { filter, arcs }
    }

    /// Same as new but doesn't check if arcs fit into the bucket.
    pub fn new_unchecked(bucket: DhtArc, arcs: Vec<DhtArc>) -> Self {
        Self {
            filter: bucket,
            arcs,
        }
    }

    #[deprecated = "use peer_view"]
    pub fn peer_view_default(&self) -> PeerViewAlpha {
        let (total, count) = self
            .arcs
            .iter()
            .fold((0u64, 0usize), |(total, count), arc| {
                (total + arc.half_length as u64, count + 1)
            });
        let average = if count > 0 {
            (total as f64 / count as f64) / MAX_HALF_LENGTH as f64
        } else {
            0.0
        };
        PeerViewAlpha::new(Default::default(), self.filter, average, count)
    }

    /// Get the density of this bucket.
    pub fn peer_view(&self, params: &PeerStrat) -> PeerView {
        match params {
            PeerStrat::Alpha(strat) => {
                let (total, count) = self
                    .arcs
                    .iter()
                    .fold((0u64, 0usize), |(total, count), arc| {
                        (total + arc.half_length as u64, count + 1)
                    });
                let average = if count > 0 {
                    (total as f64 / count as f64) / MAX_HALF_LENGTH as f64
                } else {
                    0.0
                };
                PeerViewAlpha::new(strat.clone(), self.filter, average, count).into()
            }
        }
    }
}

impl std::fmt::Display for DhtArcBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for a in &self.arcs {
            writeln!(f, "{}", a)?;
        }
        writeln!(f, "{} <- Bucket arc", self.filter)
    }
}
