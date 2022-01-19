use kitsune_p2p_dht_arc::{ArcInterval, DhtArcSet};

use super::{is_full, Arq, ArqBounds, ArqSet, ArqStrat};

pub struct PeerView {
    /// The strategy which generated this view
    strat: ArqStrat,
    /// The peers in this view (TODO: replace with calculated values)
    peers: ArqSet,
}

impl PeerView {
    pub fn new(strat: ArqStrat, arqs: ArqSet) -> Self {
        Self { strat, peers: arqs }
    }

    /// Extrapolate the coverage of the entire network from our local view.
    ///
    /// NB: This includes the filter arq, since our arc is contributing to total coverage too!
    pub fn extrapolated_coverage(&self, filter: &ArqBounds) -> f64 {
        let filter = filter.to_interval();
        if filter == ArcInterval::Empty {
            // More accurately this would be 0, but it's handy to not have
            // divide-by-zero crashes
            return 1.0;
        }
        let filter_len = filter.length();
        let base = DhtArcSet::from_interval(filter.clone());

        // FIXME: We can't just filter arcs on the fly here, because we might be
        // trying to get coverage info for an area we don't have arcs for
        // (because we don't store arcs for agents outside of our arc).
        // So, we need to extrapolate the arcs we do have to extend into the
        // unknown area outside the filter.
        // For now though, just filter arcs on the fly so we have something to test.
        // But, this means that the behavior for growing arcs is going to be a bit
        // different in the future.
        let sum = self.filtered_arqs(filter).fold(0u64, |sum, arq| {
            let arc = arq.to_interval();
            let s = DhtArcSet::from_interval(arc);
            sum + base
                .intersection(&s)
                .intervals()
                .into_iter()
                .map(|i| i.length())
                .sum::<u64>()
        });
        sum as f64 / filter_len as f64 + 1.0
    }

    /// Compute the total coverage observed within the filter interval.
    pub fn raw_coverage(&self, filter: &ArqBounds) -> f64 {
        self.extrapolated_coverage(filter) * filter.to_interval().length() as f64 / 2f64.powf(32.0)
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
    pub fn update_arq(&self, arq: &mut Arq) -> bool {
        let cov = self.extrapolated_coverage(&arq.to_bounds());

        let old_count = arq.count();
        let old_power = arq.power();
        let under = cov < self.strat.min_coverage;
        let over = cov > self.strat.max_coverage();
        let new_count = if under {
            // grow. ratio is > 1.
            // never grow by more than 2x.
            // always grow by at least 1.
            let ratio = (self.strat.midline_coverage() / cov).clamp(0.5, 2.0);
            (old_count as f64 * ratio).ceil() as u32
        } else if over {
            // shrink. ratio is < 1.
            // never shrink by more than half.
            // always shrink by at least 1.
            let ratio = (self.strat.midline_coverage() / cov).clamp(0.5, 2.0);
            (old_count as f64 * ratio).floor() as u32
        } else {
            old_count
        };

        if new_count != old_count {
            arq.count = new_count;
            let new_cov = self.extrapolated_coverage(&arq.to_bounds());

            // If this change causes us to go below the target coverage,
            // don't update. This can happen in particular when shrinking would
            // cause us to lose sight of peers.
            if over && (new_cov < self.strat.min_coverage) {
                return false;
            }
        }

        let PowerStats { median, .. } = self.power_stats(&arq);

        #[allow(unused_parens)]
        loop {
            // check for power downshift opportunity
            if (
                // not already at the minimum
                arq.power > self.strat.min_power
                // don't power down if power is already too low
                && (median as i8 - arq.power as i8) < self.strat.max_power_diff as i8
                // only power down if too few chunks
                && arq.count < self.strat.min_chunks()
            ) {
                *arq = arq.downshift();
            } else {
                break;
            }
        }

        #[allow(unused_parens)]
        loop {
            // check for power upshift opportunity
            if (
                // not already at the maximum
                arq.power < self.strat.max_power
                // don't power up if power is already too high
                && (arq.power as i8 - median as i8) < self.strat.max_power_diff as i8
                // only power up if too many chunks
                && arq.count > self.strat.max_chunks()
            ) {
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
                break;
            }
        }

        if is_full(arq.power(), arq.count()) {
            *arq = Arq::new_full(arq.center(), arq.power());
        }

        if arq.count() > self.strat.max_chunks() {
            *arq.count_mut() = self.strat.max_chunks();
        }

        // check if anything changed
        !(arq.power() == old_power && arq.count() == old_count)
    }

    pub fn power_stats(&self, filter: &Arq) -> PowerStats {
        use statrs::statistics::*;
        let mut powers: Vec<_> = self
            .filtered_arqs(filter.to_interval())
            .filter(|a| a.count > 0)
            .map(|a| a.power as f64)
            .collect();
        powers.push(filter.power as f64);
        let powers = statrs::statistics::Data::new(powers);
        let median = powers.median() as u8;
        let std_dev = powers.std_dev().unwrap_or_default();
        if std_dev > self.strat.power_std_dev_threshold {
            tracing::warn!("Large power std dev: {}", std_dev);
        }
        PowerStats { median, std_dev }
    }

    fn filtered_arqs<'a>(&'a self, filter: ArcInterval) -> impl Iterator<Item = &'a ArqBounds> {
        self.peers
            .arqs
            .iter()
            .filter(move |arq| filter.contains(arq.pseudocenter()))
    }
}

pub struct PowerStats {
    pub median: u8,
    pub std_dev: f64,
}

#[cfg(test)]
mod tests {

    use kitsune_p2p_dht_arc::ArcInterval;

    use crate::arq::pow2;

    use super::*;

    fn int(pow: u8, lo: u32, hi: u32) -> ArqBounds {
        ArqBounds::from_interval(pow, ArcInterval::new(pow2(pow) * lo, pow2(pow) * hi)).unwrap()
    }

    #[test]
    fn test_coverage() {
        let pow = 25;
        let arqs = ArqSet::new(vec![
            // 01
            //  12
            //   23
            int(pow, 0, 0x20),
            int(pow, 0x10, 0x30),
            int(pow, 0x20, 0x40),
        ]);
        let view = PeerView::new(Default::default(), arqs);
        assert_eq!(view.extrapolated_coverage(&int(pow, 0, 0x10)), 2.0);
        assert_eq!(view.extrapolated_coverage(&int(pow, 0, 0x20)), 2.5);
        assert_eq!(view.extrapolated_coverage(&int(pow, 0, 0x40)), 2.5);

        // All arcs get filtered out, so this would normally be 3, but actually
        // it's simply 1.
        assert_eq!(view.extrapolated_coverage(&int(pow, 0x10, 0x20)), 1.0);
    }
}
