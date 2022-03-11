use crate::*;

/// The default, and only, strategy for generating a PeerView
#[derive(Debug, Clone, Copy)]
pub struct PeerStratBeta {
    pub focus_nearby: bool,
    pub min_sample_size: u16,
    pub coverage_buffer: f64,
    pub total_coverage_buffer: f64,
    pub default_uptime: f64,
    pub noise_threshold: f64,
    pub delta_scale: f64,
    pub delta_threshold: f64,
}

impl Default for PeerStratBeta {
    fn default() -> Self {
        Self {
            focus_nearby: true,
            min_sample_size: DEFAULT_REDUNDANCY_TARGET as u16,
            coverage_buffer: DEFAULT_COVERAGE_BUFFER,
            total_coverage_buffer: DEFAULT_TOTAL_COVERAGE_BUFFER,
            default_uptime: DEFAULT_UPTIME,
            noise_threshold: DEFAULT_NOISE_THRESHOLD,
            delta_scale: DEFAULT_DELTA_SCALE,
            delta_threshold: DEFAULT_DELTA_THRESHOLD,
        }
    }
}

impl PeerStratBeta {
    pub fn view(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerViewBeta {
        let peers: Vec<DhtArc> = peers
            .iter()
            .filter(|a| arc.contains(a.start_loc()))
            .copied()
            .collect();
        Self::view_unchecked(self, arc, peers.as_slice())
    }

    pub fn view_unchecked(&self, arc: DhtArc, peers: &[DhtArc]) -> PeerViewBeta {
        let total_coverage = total_coverage(peers);
        let count = peers.len();
        let mut full_view = PeerViewBeta::new(*self, arc, total_coverage, count);
        if let Some(focused_view) =
            self.check_focused_view(arc, peers, self.min_sample_size as usize)
        {
            full_view.focused_view_target = Some(focused_view);
        }
        full_view
    }

    /// Check a more focused view of the network if we are currently viewing a much larger
    /// portion of the network then we might need to.
    ///
    /// The idea here is that you may already have enough peer information to make a
    /// better extrapolation of the full network in a area more focused around your location
    /// point. In this case it is not worth waiting for all the peer information to
    /// sync for the larger view as that might take a long time.
    ///
    /// Imagine the case where you join a large network with a full arc, you might have a
    /// good sample size close to your location long before you sync all the networks peer
    /// information.
    fn check_focused_view(&self, arc: DhtArc, peers: &[DhtArc], focus_size: usize) -> Option<f64> {
        // Focus size cannot be zero.
        if focus_size == 0 {
            return None;
        }
        // If view focusing is on and our peer count is twice the size of the focus
        // size then check a more focused view.
        (self.focus_nearby && peers.len() >= focus_size as usize * 2)
            .then(|| {
                // Sort the peers by distance to this view's start location.
                let mut closest_peers = peers.to_vec();
                closest_peers
                    .sort_unstable_by_key(|a| wrapped_distance(arc.start_loc(), a.start_loc()));

                // Take a focused sample of the min_sample_size.
                let closest_peers = closest_peers
                    .into_iter()
                    .take(focus_size)
                    .collect::<Vec<_>>();

                // Create the focused view's arc using the
                // furthest peer as the half length.
                let focused_arc = DhtArc::from_start_and_half_len(
                    arc.start_loc(),
                    wrapped_distance(
                        arc.start_loc(),
                        closest_peers
                            .last()
                            .expect("Can't be empty if we have twice the focus size")
                            .start_loc(),
                    )
                    .into(),
                );

                // Disable focusing to prevent recursion.
                let mut strat = *self;
                strat.focus_nearby = false;

                // Create the focused view.
                strat.view_unchecked(focused_arc, &closest_peers)
            })
            .and_then(|focused_view| {
                // If the estimated total coverage of the more focused view is greater
                // then our target network coverage then we will use the target coverage
                // of the focused view.
                (focused_view.est_total_coverage() >= self.target_network_coverage())
                    .then(|| focused_view.target_coverage())
            })
    }

    /// The target coverage of the network so that it
    /// has enough redundancy.
    pub fn target_network_coverage(&self) -> f64 {
        // For this strategy we are aiming to have the
        // same amount as coverage as a network with our
        // minimum sample size if all peers had a coverage of
        // 1.0 or 100%.
        //
        // So this can be thought of as number of peers * 1.0
        // where the number of peers is our minimum sample size.
        self.min_sample_size as f64
    }
}

/// An alternative PeerView.
#[derive(Debug, Clone, Copy)]
pub struct PeerViewBeta {
    /// The strategy params that generated this view.
    pub strat: PeerStratBeta,
    /// The arc that filtered the bucket that generated this view.
    filter: DhtArc,
    /// The number of peers in the bucket.
    pub count: usize,
    /// An optional more focused view's target
    /// coverage for this view.
    focused_view_target: Option<f64>,
    /// The total coverage found in this view.
    total_coverage: f64,
}

impl PeerViewBeta {
    /// Create a new peer view reading from the:
    /// - The filter used to create the bucket.
    /// - Average coverage of all peers in the bucket.
    /// - Count of peers in the bucket.
    pub fn new(strat: PeerStratBeta, filter: DhtArc, total_coverage: f64, count: usize) -> Self {
        Self {
            strat,
            filter,
            count,
            focused_view_target: None,
            total_coverage,
        }
    }

    /// Calculate the target arc length based on this view.
    pub fn target_coverage(&self) -> f64 {
        // If we haven't observed at least our minimum sample size
        // of peers then we know that the data then we can't make
        // good extrapolations of the network so need to grow
        // to hold at least that many peers.
        if !self.has_min_sample_size() {
            1.0
        } else {
            // If we have a more focused view target then use that.
            if let Some(focused_view_target) = self.focused_view_target {
                return focused_view_target;
            }

            // Target the difference between the target network coverage
            // and the estimated network coverage.
            // A positive number is the amount of coverage we estimate that
            // is missing.
            // 0.0 means we have too much coverage.
            // let target =
            //     (self.strat.target_network_coverage() - self.est_total_coverage()).max(0.0);
            let target = self.strat.target_network_coverage() - self.est_total_coverage();

            // A buffer to allow for estimation errors.
            let estimation_error_buffer =
                self.strat.total_coverage_buffer * self.strat.target_network_coverage();

            // let target = if target > estimation_error_buffer {
            let target = if target > 0.0 {
                // If we estimate that we need more then the error buffer then
                // we have too little coverage.

                // We aim for the maximum of the ideal target and the estimated
                // missing coverage.
                target.max(self.ideal_target())
            // } else if target == 0.0 {
            } else if target <= -estimation_error_buffer {
                // If we estimate the network has too much
                // coverage (no missing coverage)
                // then we should shrink our storage arc.
                0.0
            } else {
                // If we are within the estimation
                // error buffer then we can assume the
                // network has just the right amount of coverage.
                //
                // It is hard to say what we should do here but targeting
                // our ideal target coverage means we will shrink down to
                // a sensible coverage size.

                self.ideal_target()
            };

            target.clamp(0.0, 1.0)
        }
    }

    /// Given the current coverage, what is the next step to take in reaching
    /// the ideal coverage?
    pub fn next_coverage(&self, current: f64) -> f64 {
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
        // If this is below our threshold then go straight to the target.
        if delta.abs() < self.strat.delta_threshold {
            target
        // Other wise scale the delta to avoid rapid change.
        } else {
            current + (delta * self.strat.delta_scale)
        }
    }

    /// The expected number of peers for this arc over time.
    pub fn expected_count(&self) -> usize {
        (self.count as f64 * self.strat.default_uptime) as usize
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

    /// Estimated total coverage on the network if all other views
    /// are similar to this view.
    pub fn est_total_coverage(&self) -> f64 {
        let coverage = self.filter.coverage();
        if coverage > 0.0 {
            (1.0 / coverage) * self.total_coverage * self.strat.default_uptime
        } else {
            // If we had no coverage when we collected these
            // peers then we can't make a good guess at the total.
            0.0
        }
    }

    /// Has this view met the minimum sample size of peers?
    pub(crate) fn has_min_sample_size(&self) -> bool {
        self.count >= (self.strat.min_sample_size as usize)
    }

    /// The ideal coverage if all peers were holding the same sized
    /// arcs and our estimated total peers is close.
    pub(crate) fn ideal_target(&self) -> f64 {
        let est_total_peers = self.est_total_peers();
        if est_total_peers <= self.strat.min_sample_size as usize {
            1.0
        } else {
            self.strat.min_sample_size as f64 / est_total_peers as f64
        }
    }
}

/// Total coverage of all peers.
fn total_coverage(peers: &[DhtArc]) -> f64 {
    peers.iter().map(|a| a.coverage()).sum::<f64>()
}
