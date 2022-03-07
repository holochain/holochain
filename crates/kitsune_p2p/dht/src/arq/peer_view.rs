use kitsune_p2p_dht_arc::{ArcInterval, DhtArc, PeerViewAlpha, PeerViewBeta};
use num_traits::Zero;

use crate::quantum::Topology;

use super::{is_full, Arq, ArqBounded, ArqBounds, ArqStrat};

/// A "view" of the peers in a neighborhood. The view consists of a few
/// observations about the distribution of peers within a particular arc, used
/// to make inferences about the rest of the (out-of-view) DHT, ultimately
/// enabling the calculation of the target arc size for the agent who has this View.
///
/// The enum allows us to add different views (and different calculations of
/// target arc length) over time.
#[derive(derive_more::From)]
pub enum PeerView {
    Alpha(PeerViewAlpha),
    Beta(PeerViewBeta),
    Quantized(PeerViewQ),
}

impl PeerView {
    /// Given the current view of a peer and the peer's current coverage,
    /// this returns the next step to take in reaching the ideal coverage.
    pub fn update_arc(&self, dht_arc: &mut DhtArc) -> bool {
        match self {
            Self::Alpha(v) => v.update_arc(dht_arc),
            Self::Beta(v) => v.update_arc(dht_arc),
            Self::Quantized(v) => {
                let mut arq = Arq::from_dht_arc(&v.strat, dht_arc);
                let updated = v.update_arq(&mut arq);
                *dht_arc = arq.to_dht_arc(&v.topo);
                updated
            }
        }
    }
}

pub struct PeerViewQ {
    /// The strategy which generated this view
    strat: ArqStrat,

    /// The topology of the network space
    topo: Topology,

    /// The peers in this view (TODO: replace with calculated values)
    peers: Vec<Arq>,

    #[cfg(feature = "testing")]
    /// Omit the arq at this index from all peer considerations.
    /// Useful for tests which update all arqs, without needing to
    /// construct a new PeerView for each arq needing to be updated
    pub skip_index: Option<usize>,
}

impl PeerViewQ {
    pub fn new(topo: Topology, strat: ArqStrat, peers: Vec<Arq>) -> Self {
        Self {
            strat,
            topo,
            peers,
            #[cfg(feature = "testing")]
            skip_index: None,
        }
    }

    /// The actual coverage of all arcs in this view.
    /// TODO: this only makes sense when the view contains all agents in the DHT.
    ///       So, it's more useful for testing. Probably want to tease out some
    ///       concept of a test DHT from this.
    pub fn actual_coverage(&self) -> f64 {
        actual_coverage(&self.topo, self.peers.iter())
    }

    /// Extrapolate the coverage of the entire network from our local view.
    pub fn extrapolated_coverage(&self, filter: &ArqBounds) -> f64 {
        self.extrapolated_coverage_and_filtered_count(filter).0
    }

    /// Return the extrapolated coverage and the number of arqs which match the filter.
    /// These two are complected together simply for efficiency's sake, to
    /// minimize computation
    ///
    /// TODO: this probably will be rewritten when PeerView is rewritten to
    /// have the filter baked in.
    pub fn extrapolated_coverage_and_filtered_count(&self, filter: &ArqBounds) -> (f64, usize) {
        let filter = filter.to_interval(&self.topo);
        if filter == ArcInterval::Empty {
            // More accurately this would be 0, but it's handy to not have
            // divide-by-zero crashes
            return (1.0, 1);
        }
        let filter_len = filter.length();

        let initial = (0, 0);

        // FIXME: We can't just filter arcs on the fly here, because we might be
        // trying to get coverage info for an area we don't have arcs for
        // (because we don't store arcs for agents outside of our arc).
        // So, we need to extrapolate the arcs we do have to extend into the
        // unknown area outside the filter.
        // For now though, just filter arcs on the fly so we have something to test.
        // But, this means that the behavior for growing arcs is going to be a bit
        // different in the future.
        let (sum, count) = self
            .filtered_arqs(filter)
            .fold(initial, |(sum, count), arq| {
                (sum + arq.length(&self.topo), count + 1)
            });
        let cov = sum as f64 / filter_len as f64;
        (cov, count)
    }

    /// Compute the total coverage observed within the filter interval.
    pub fn raw_coverage(&self, filter: &ArqBounds) -> f64 {
        self.extrapolated_coverage(filter) * filter.to_interval(&self.topo).length() as f64
            / 2f64.powf(32.0)
    }

    pub fn update_arq(&self, arq: &mut Arq) -> bool {
        self.update_arq_with_stats(arq).changed
    }

    fn is_slacking(&self, cov: f64, num_peers: usize) -> bool {
        num_peers as f64 <= cov * self.strat.slacker_ratio
    }

    pub fn slack_factor(&self, cov: f64, num_peers: usize) -> f64 {
        if self.is_slacking(cov, num_peers) {
            if num_peers.is_zero() {
                // Prevent a NaN.
                // This value gets clamped anyway, so it will never actually
                // lead to an infinite value.
                f64::INFINITY
            } else {
                cov / num_peers as f64
            }
        } else {
            1.0
        }
    }

    fn growth_factor(&self, cov: f64, num_peers: usize, median_power_diff: i8) -> f64 {
        let np = num_peers as f64;
        let under = cov < self.strat.min_coverage;
        let over = cov > self.strat.max_coverage();

        // The ratio of ideal coverage vs actual observed coverage.
        // A ratio > 1 indicates undersaturation and a need to grow.
        // A ratio < 1 indicates oversaturation and a need to shrink.
        let cov_diff = if over || under {
            let ratio = self.strat.midline_coverage() / cov;

            // We want to know which of our peers are likely to be making a similar
            // update to us, because that will affect the overall coverage more
            // than the drop in the bucket that we can provide.
            //
            // If all peers have seen the same change as us since their last update,
            // they will on average move similarly to us, and so we should only make
            // a small step in the direction of the target, trusting that our peers
            // will do the same.
            //
            // Conversely, if all peers are stable, e.g. if we just came online to
            // find a situation where all peers around us are under-representing,
            // but stable, then we want to make a much bigger leap.
            let peer_dampening_factor = 1.0 / (1.0 + np);

            (ratio - 1.0) * peer_dampening_factor + 1.0
        } else {
            1.0
        };

        // The "slacker" factor. If our observed coverage is significantly
        // greater than the number of peers we see, it's an indication
        // that we may need to pick up more slack.
        //
        // This check helps balance out stable but unequitable situations where
        // all peers have a similar estimated coverage, but some peers are
        // holding much more than others.
        let slack_factor = self.slack_factor(cov, num_peers);

        let unbounded_growth = cov_diff * slack_factor;

        // The difference between the median power and the arq's power helps
        // determine some limits on growth.
        // If we are at the median growth, then it makes sense to cap
        // our movement by 2x in either direction (1/2 to 2)
        //
        // If we are 1 below the median, then our range is (1/2 to 4)
        // If we are 2 below the median, then our range is (1/2 to 8)
        // If we are 1 above the median, then our range is (1/4 to 2)
        // If we are 2 above the median, then our range is (1/8 to 2)
        //
        // Note that there is also a hard limit on growth described by
        // ArqStrat::max_power_diff, enforced elsewhere.
        let mpd = median_power_diff as f64;
        let min = 2f64.powf(mpd).min(0.5);
        let max = 2f64.powf(mpd).max(2.0);
        unbounded_growth.clamp(min, max)
    }

    /// Take an arq and potentially resize and requantize it based on this view.
    ///
    /// This represents an iterative step towards the ideal coverage, based on
    /// the observed coverage.
    /// This makes many assumptions, including:
    /// - this arc resizing algorithm is a good one, namely that the coverage
    ///     at any point of the DHT is close to the target range
    /// - all other peers are following the same algorithm
    /// - if we see a change that we need to make, we assume that a number of
    ///     peers are about to make a similar change, and that number is on
    ///     average the same as our target coverage
    ///
    /// More detail on these assumptions here:
    /// https://hackmd.io/@hololtd/r1IAIbr5Y/https%3A%2F%2Fhackmd.io%2FK_fkBj6XQO2rCUZRRL9n2g
    pub fn update_arq_with_stats(&self, arq: &mut Arq) -> UpdateArqStats {
        let (cov, num_peers) = self.extrapolated_coverage_and_filtered_count(&arq.to_bounds());

        let old_count = arq.count();
        let old_power = arq.power();

        let power_stats = self.power_stats(&arq);
        let PowerStats {
            median: median_power,
            ..
        } = power_stats;

        let median_power_diff = median_power as i8 - arq.power() as i8;
        let growth_factor = self.growth_factor(cov, num_peers, median_power_diff);

        let new_count = if growth_factor < 1.0 {
            // Ensure we shrink by at least 1
            (old_count as f64 * growth_factor).floor() as u32
        } else {
            // Ensure we grow by at least 1 (if there is any growth at all)
            (old_count as f64 * growth_factor).ceil() as u32
        };

        if new_count != old_count {
            let mut tentative = arq.clone();
            tentative.count = new_count;

            // If shrinking caused us to go below the target coverage,
            // or to start "slacking" (not seeing enough peers), then
            // don't update. This happens when we shrink too much and
            // lose sight of peers.
            let (new_cov, new_num_peers) =
                self.extrapolated_coverage_and_filtered_count(&tentative.to_bounds());
            if new_count < old_count
                && (new_cov < self.strat.min_coverage
                    || (!self.is_slacking(cov, num_peers)
                        && self.is_slacking(new_cov, new_num_peers)))
            {
                return UpdateArqStats {
                    changed: false,
                    desired_delta: new_count as i32 - old_count as i32,
                    power: None,
                    num_peers,
                };
            }
        }

        // Commit the change to the count
        arq.count = new_count;

        let power_above_min = |pow| {
            // not already at the minimum
            pow > self.strat.min_power
             // don't power down if power is already too low
             && (median_power as i8 - pow as i8) < self.strat.max_power_diff as i8
        };

        loop {
            // check for power downshift opportunity
            if arq.count < self.strat.min_chunks() {
                if power_above_min(arq.power) {
                    *arq = arq.downshift();
                } else {
                    // If we could not downshift due to other constraints, then we cannot
                    // shrink any smaller than the min_chunks.
                    arq.count = self.strat.min_chunks();
                }
            } else {
                break;
            }
        }

        let power_below_max = |pow| {
            // not already at the maximum
            pow < self.strat.max_power
            // don't power up if power is already too high
            && (pow as i8 - median_power as i8) < self.strat.max_power_diff as i8
        };

        loop {
            // check for power upshift opportunity
            if arq.count > self.strat.max_chunks() {
                if power_below_max(arq.power) {
                    // Attempt to requantize to the next higher power.
                    // If we only grew by one chunk, into an odd count, then don't
                    // force upshifting, because that would either require undoing
                    // the growth, or growing by 2 instead of 1. In this case, skip
                    // upshifting, and we'll upshift on the next update.
                    let force = new_count as i32 - old_count as i32 > 1;
                    if let Some(a) = arq.upshift(force) {
                        *arq = a
                    } else {
                        break;
                    }
                } else {
                    // If we could not upshift due to other constraints, then we cannot
                    // grow any larger than the max_chunks.
                    arq.count = self.strat.max_chunks();
                }
            } else {
                break;
            }
        }

        if is_full(arq.power(), arq.count()) {
            *arq = Arq::new_full(arq.center(), arq.power());
        }

        // check if anything changed
        let changed = !(arq.power() == old_power && arq.count() == old_count);

        UpdateArqStats {
            changed,
            desired_delta: new_count as i32 - old_count as i32,
            power: Some(power_stats),
            num_peers,
        }
    }

    pub fn power_stats(&self, filter: &Arq) -> PowerStats {
        use statrs::statistics::*;
        let mut powers: Vec<_> = self
            .filtered_arqs(filter.to_interval(&self.topo))
            .filter(|a| a.count > 0)
            .map(|a| a.power as f64)
            .collect();
        powers.push(filter.power() as f64);
        let powers = statrs::statistics::Data::new(powers);
        let median = powers.median() as u8;
        let std_dev = powers.std_dev().unwrap_or_default();
        if std_dev > self.strat.power_std_dev_threshold {
            // tracing::warn!("Large power std dev: {}", std_dev);
        }
        PowerStats { median, std_dev }
    }

    fn filtered_arqs<'a>(&'a self, filter: ArcInterval) -> impl Iterator<Item = &'a Arq> {
        let it = self.peers.iter();

        #[cfg(feature = "testing")]
        let it = it
            .enumerate()
            .filter(|(i, _)| self.skip_index.as_ref() != Some(i))
            .map(|(_, arq)| arq);

        it.filter(move |arq| filter.contains(arq.center))
    }
}

#[derive(Debug, Clone)]
pub struct UpdateArqStats {
    pub changed: bool,
    pub desired_delta: i32,
    pub power: Option<PowerStats>,
    pub num_peers: usize,
}

/// The actual coverage provided by these peers. Assumes that this is the
/// entire view of the DHT, all peers are accounted for here.
pub fn actual_coverage<'a, A: 'a, P: Iterator<Item = &'a A>>(topo: &Topology, peers: P) -> f64
where
    ArqBounds: From<&'a A>,
{
    peers
        .map(|a| ArqBounds::from(a).length(topo) as f64 / 2f64.powf(32.0))
        .sum()
}

#[derive(Debug, Clone)]
pub struct PowerStats {
    pub median: u8,
    pub std_dev: f64,
}

#[cfg(test)]
mod tests {

    use kitsune_p2p_dht_arc::ArcInterval;

    use crate::arq::{pow2, print_arqs};
    use crate::quantum::Topology;
    use crate::Loc;

    use super::*;

    fn make_arq(pow: u8, lo: u32, hi: u32) -> Arq {
        ArqBounds::from_interval_rounded(
            pow,
            ArcInterval::new(pow2(pow) * lo, (pow2(pow) as u64 * hi as u64) as u32),
        )
        .to_arq()
    }

    #[test]
    fn test_filtered_arqs() {
        let topo = Topology::identity_zero();
        let pow = 25;
        let s = pow2(pow);
        let a = make_arq(pow, 0, 0x20);
        let b = make_arq(pow, 0x10, 0x30);
        let c = make_arq(pow, 0x20, 0x40);
        assert_eq!(a.center, Loc::from(s * 0x0 + s / 2));
        assert_eq!(b.center, Loc::from(s * 0x10 + s / 2));
        assert_eq!(c.center, Loc::from(s * 0x20 + s / 2));
        let arqs = vec![a, b, c];
        print_arqs(&topo, &arqs, 64);
        let view = PeerViewQ::new(topo.clone(), Default::default(), arqs);

        let get = |b: Arq| {
            view.filtered_arqs(b.to_interval(&topo))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(get(make_arq(pow, 0, 0x10)), vec![a]);
        assert_eq!(get(make_arq(pow, 0, 0x20)), vec![a, b]);
        assert_eq!(get(make_arq(pow, 0, 0x40)), vec![a, b, c]);
        assert_eq!(get(make_arq(pow, 0x10, 0x20)), vec![b]);
    }

    #[test]
    fn test_coverage() {
        let topo = Topology::identity_zero();
        let pow = 24;
        let arqs: Vec<_> = (0..0x100)
            .step_by(0x10)
            .map(|x| make_arq(pow, x, x + 0x20))
            .collect();
        print_arqs(&topo, &arqs, 64);
        let view = PeerViewQ::new(topo, Default::default(), arqs);
        assert_eq!(
            view.extrapolated_coverage_and_filtered_count(&make_arq(pow, 0, 0x10).to_bounds()),
            (2.0, 1)
        );
        assert_eq!(
            view.extrapolated_coverage_and_filtered_count(&make_arq(pow, 0, 0x20).to_bounds()),
            (2.0, 2)
        );
        assert_eq!(
            view.extrapolated_coverage_and_filtered_count(&make_arq(pow, 0, 0x40).to_bounds()),
            (2.0, 4)
        );

        // TODO: when changing PeerView logic to bake in the filter,
        // this will probably change
        assert_eq!(
            view.extrapolated_coverage_and_filtered_count(&make_arq(pow, 0x10, 0x20).to_bounds()),
            (2.0, 1)
        );
    }
}
