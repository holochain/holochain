use kitsune_p2p_dht_arc::{ArcInterval, DhtArcSet};

use crate::arq::is_full;

use super::{Arq, ArqBounds, ArqSet, ArqStrat};

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
    /// NB: This includes the filter arq, since we are contributing to coverage too!
    pub fn extrapolated_coverage(&self, filter: &ArqBounds) -> f64 {
        let filter = filter.to_interval();
        let filter_len = filter.length();
        let base = DhtArcSet::from_interval(filter.clone());
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
    pub fn update_arq(&self, mut arq: Arq) -> Arq {
        arq = self.update_power(arq);
        arq = self.update_size(arq);
        arq
    }

    fn update_size(&self, mut arq: Arq) -> Arq {
        let bounds = ArqBounds::from_arq(arq.clone());
        let extrapolated_coverage = self.extrapolated_coverage(&bounds);
        if extrapolated_coverage < self.strat.min_coverage {
            arq.count += 1;
        } else if extrapolated_coverage > self.strat.max_coverage() {
            // shrink
            arq.count -= 1;
        };
        arq
    }

    #[allow(unused_parens)]
    fn update_power(&self, mut arq: Arq) -> Arq {
        // NOTE: these stats will be baked into the view eventually.
        let PowerStats { median, .. } = self.power_stats(&arq);

        // check for power downshift opportunity
        loop {
            if (
                // not already at the minimum
                arq.power > self.strat.min_power
                // don't power down if power is already too low
                && (median as i8 - arq.power as i8) < self.strat.max_power_diff as i8
                // only power down if too few chunks
                && arq.count < self.strat.min_chunks()
                // attempt to requantize (cannot fail for downshift)
                && arq.requantize(arq.power - 1)
            ) {
                // we downshifted!
            } else {
                break;
            }
        }

        // check for power upshift opportunity
        loop {
            if (
                // not already at the maximum
                arq.power < u8::MAX
                // don't power up if power is already too high
                && (arq.power as i8 - median as i8) < self.strat.max_power_diff as i8
                // only power up if too many chunks
                && arq.count > self.strat.max_chunks()
                // attempt to requantize (this may fail if chunk count is odd)
                && arq.requantize(arq.power + 1)
            ) {
                // we upshifted!
            } else {
                break;
            }
        }
        arq
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

    use super::*;

    fn int(pow: u8, lo: u32, hi: u32) -> ArqBounds {
        ArqBounds::from_interval(pow, ArcInterval::new(lo, hi)).unwrap()
    }

    #[test]
    fn test_coverage() {
        let view = PeerView::new(
            Default::default(),
            ArqSet::new(vec![
                // 01
                //  12
                //   23
                int(4, 0, 0x20),
                int(4, 0x10, 0x30),
                int(4, 0x20, 0x40),
            ]),
        );
        assert_eq!(view.extrapolated_coverage(&int(4, 0, 0x10)), 1.0);
        assert_eq!(view.extrapolated_coverage(&int(4, 0, 0x20)), 1.5);
        assert_eq!(view.extrapolated_coverage(&int(4, 0, 0x40)), 1.5);
        assert_eq!(view.extrapolated_coverage(&int(4, 0x10, 0x20)), 2.0);
    }
}
