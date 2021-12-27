use kitsune_p2p_dht_arc::DhtArcSet;

use super::{ArqBounds, ArqSet};

pub struct PeerView {
    arqs: ArqSet,
}

impl PeerView {
    pub fn new(arqs: ArqSet) -> Self {
        Self { arqs }
    }

    /// Extrapolate the coverage of the entire network from our local view.
    /// NB: currently this is no different than the actual observed coverage.
    pub fn extrapolated_coverage(&self, filter: &ArqBounds) -> f64 {
        self.coverage(filter)
    }

    /// Compute the total coverage observed within the filter interval.
    pub fn coverage(&self, filter: &ArqBounds) -> f64 {
        let filter = filter.to_interval();
        let filter_len = filter.length();
        let base = DhtArcSet::from_interval(filter);
        let sum = self.arqs.0.iter().fold(0u64, |sum, arq| {
            let arc = arq.to_interval();
            let s = DhtArcSet::from_interval(arc);
            sum + base
                .intersection(&s)
                .intervals()
                .into_iter()
                .map(|i| i.length())
                .sum::<u64>()
        });
        sum as f64 / filter_len as f64
    }
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
            vec![
                // 01
                //  12
                //   23
                int(4, 0, 0x20),
                int(4, 0x10, 0x30),
                int(4, 0x20, 0x40),
            ]
            .into(),
        );
        assert_eq!(view.coverage(&int(4, 0, 0x10)), 1.0);
        assert_eq!(view.coverage(&int(4, 0, 0x20)), 1.5);
        assert_eq!(view.coverage(&int(4, 0, 0x40)), 1.5);
        assert_eq!(view.coverage(&int(4, 0x10, 0x20)), 2.0);
    }
}
