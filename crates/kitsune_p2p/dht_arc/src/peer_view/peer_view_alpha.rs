use crate::*;

/// The default, and only, strategy for generating a PeerView
#[derive(Debug, Clone, Copy)]
pub struct PeerStratAlpha {
    pub check_gaps: bool,
    pub redundancy_target: u16,
    pub coverage_buffer: f64,
    pub default_uptime: f64,
    pub noise_threshold: f64,
    pub delta_scale: f64,
    pub delta_threshold: f64,
}

impl Default for PeerStratAlpha {
    fn default() -> Self {
        Self {
            check_gaps: true,
            redundancy_target: DEFAULT_REDUNDANCY_TARGET as u16,
            coverage_buffer: DEFAULT_COVERAGE_BUFFER,
            default_uptime: DEFAULT_UPTIME,
            noise_threshold: DEFAULT_NOISE_THRESHOLD,
            delta_scale: DEFAULT_DELTA_SCALE,
            delta_threshold: DEFAULT_DELTA_THRESHOLD,
        }
    }
}

impl PeerStratAlpha {
    pub fn view(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerViewAlpha {
        let peers: Vec<DhtArc> = peers
            .iter()
            .filter(|a| arc.contains(a.start_loc()))
            .copied()
            .collect();
        Self::view_unchecked(self, arc, peers.as_slice())
    }

    pub fn view_unchecked(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerViewAlpha {
        let (total, count) = peers.iter().fold((0u64, 0usize), |(total, count), arc| {
            (total + arc.length(), count + 1)
        });
        let average = if count > 0 {
            (total as f64 / count as f64) / U32_LEN as f64
        } else {
            0.0
        };
        PeerViewAlpha::new(*self, arc, average, count)
    }
}

/// The default, and only, PeerView.
#[derive(Debug, Clone, Copy)]
pub struct PeerViewAlpha {
    /// The strategy params that generated this view.
    strat: PeerStratAlpha,
    /// The arc that filtered the bucket that generated this view.
    filter: DhtArc,
    /// The average coverage of peers in the bucket.
    average_coverage: f64,
    /// The number of peers in the bucket.
    count: usize,
}

impl PeerViewAlpha {
    /// Create a new peer view reading from the:
    /// - The filter used to create the bucket.
    /// - Average coverage of all peers in the bucket.
    /// - Count of peers in the bucket.
    pub fn new(strat: PeerStratAlpha, filter: DhtArc, average_coverage: f64, count: usize) -> Self {
        Self {
            strat,
            filter,
            average_coverage,
            count,
        }
    }

    /// Calculate the target arc length based on this view.
    pub(crate) fn target_coverage(&self) -> f64 {
        // Get the estimated coverage gap based on our observed peer view.
        let est_gap = self.est_gap();
        // If we haven't observed at least our redundancy target number
        // of peers (adjusted for expected uptime) then we know that the data
        // in our arc is under replicated and we should start aiming for full coverage.
        if self.expected_count() < self.strat.redundancy_target as usize {
            1.0
        } else {
            // Get the estimated gap. We don't care about negative gaps
            // or gaps we can't fill (> 1.0)
            let est_gap = clamp(0.0, 1.0, est_gap);
            // Get the ideal coverage target for the size of that we estimate
            // the network to be.
            let ideal_target =
                coverage_target(self.est_total_peers(), self.strat.redundancy_target);
            // Take whichever is larger. We prefer nodes to target the ideal
            // coverage but if there is a larger gap then it needs to be filled.
            let target = est_gap.max(ideal_target);

            clamp(0.0, 1.0, target)
        }
    }

    /// Given the current coverage, what is the next step to take in reaching
    /// the ideal coverage?
    pub fn update_arc(&self, dht_arc: &mut DhtArc) -> bool {
        let current = dht_arc.coverage();
        let target = {
            let target_lo = self.target_coverage();
            let target_hi = (target_lo + self.strat.coverage_buffer).min(1.0);

            if current < target_lo {
                target_lo
            } else if current > target_hi {
                target_hi
            } else {
                current
            }
        };

        // The change in arc we'd need to make to get to the target.
        let delta = target - current;
        if delta > 0.0 {
            // If this is below our threshold then go straight to the target.
            let new_coverage = if delta.abs() < self.strat.delta_threshold {
                target
            // Other wise scale the delta to avoid rapid change.
            } else {
                current + (delta * self.strat.delta_scale)
            };
            dht_arc.update_length((U32_LEN as f64 * new_coverage) as u64);
            true
        } else {
            false
        }
    }

    /// The expected number of peers for this arc over time.
    pub fn expected_count(&self) -> usize {
        (self.count as f64 * self.strat.default_uptime) as usize
    }

    /// Estimate the gap in coverage that needs to be filled.
    /// If the gap is negative that means we are over covered.
    pub fn est_gap(&self) -> f64 {
        if !self.strat.check_gaps {
            return 0.0;
        }
        let est_total_peers = self.est_total_peers();
        let ideal_target = coverage_target(est_total_peers, self.strat.redundancy_target);
        let gap = ideal_target - self.average_coverage;
        // We want to check the ratio between the gap and the target
        // because small targets will have small gaps.
        let gap_ratio = gap.abs() / ideal_target;
        if gap_ratio < self.strat.noise_threshold {
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

// TODO: Use the [`f64::clamp`] when we switch to rustc 1.50
pub(crate) fn clamp(min: f64, max: f64, mut x: f64) -> f64 {
    if x < min {
        x = min;
    }
    if x > max {
        x = max;
    }
    x
}

/// The ideal coverage if all peers were holding the same sized
/// arcs and our estimated total peers is close.
pub(crate) fn coverage_target(est_total_peers: usize, redundancy_target: u16) -> f64 {
    if est_total_peers <= redundancy_target as usize {
        1.0
    } else {
        redundancy_target as f64 / est_total_peers as f64
    }
}
