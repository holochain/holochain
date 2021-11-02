use super::{coverage_target, DhtArc, DEFAULT_UPTIME, MAX_HALF_LENGTH, NOISE_THRESHOLD};

#[derive(Debug, Clone)]
pub enum PeerViewParams {
    Alpha,
}

#[derive(Debug, Clone, derive_more::From)]
pub enum PeerView {
    Alpha(PeerViewAlpha),
}

#[derive(Debug, Clone, Copy)]
/// The average density of peers at a location in the u32 space.
pub struct PeerViewAlpha {
    /// The arc that filtered the bucket that generated this density.
    filter: DhtArc,
    /// The average coverage of peers in the bucket.
    average_coverage: f64,
    /// The number of peers in the bucket.
    count: usize,
}

impl PeerViewAlpha {
    /// Create a new peer density reading from the:
    /// - The filter used to create the bucket.
    /// - Average coverage of all peers in the bucket.
    /// - Count of peers in the bucket.
    pub fn new(filter: DhtArc, average_coverage: f64, count: usize) -> Self {
        Self {
            filter,
            average_coverage,
            count,
        }
    }

    /// The expected number of peers for this arc over time.
    pub fn expected_count(&self) -> usize {
        (self.count as f64 * DEFAULT_UPTIME) as usize
    }

    /// Estimate the gap in coverage that needs to be filled.
    /// If the gap is negative that means we are over covered.
    pub fn est_gap(&self) -> f64 {
        let est_total_peers = self.est_total_peers();
        let ideal_target = coverage_target(est_total_peers);
        let gap = ideal_target - self.average_coverage;
        // We want to check the ratio between the gap and the target
        // because small targets will have small gaps.
        let gap_ratio = gap.abs() / ideal_target;
        if gap_ratio < NOISE_THRESHOLD {
            0.0
        } else {
            gap * est_total_peers as f64
        }
    }

    /// Estimate total peers.
    pub fn est_total_peers(&self) -> usize {
        let coverage = self.filter.coverage();
        if coverage > 0.0 {
            (1.0 / coverage * self.expected_count() as f64) as usize
        } else {
            // If we had no coverage when we collected these
            // peers then we can't make a good guess at the total.
            0
        }
    }

    /// Estimated total redundant coverage.
    pub fn est_total_redundancy(&self) -> usize {
        (self.est_total_peers() as f64 * self.average_coverage) as usize
    }
}

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
    pub fn peer_view_alpha(&self) -> PeerViewAlpha {
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
        PeerViewAlpha::new(self.filter, average, count)
    }

    /// Get the density of this bucket.
    pub fn peer_view(&self, params: &PeerViewParams) -> PeerView {
        match params {
            PeerViewParams::Alpha => {
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
                PeerViewAlpha::new(self.filter, average, count).into()
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
