//! Utils for testing

mod gossip_direct;
mod min_redundancy;
mod op_data;
mod op_store;
mod test_node;

pub use gossip_direct::*;
pub use min_redundancy::*;
pub use op_data::*;
pub use op_store::*;
pub use test_node::*;

use crate::arq::*;
use crate::spacetime::Topology;

use kitsune_p2p_dht_arc::DhtLocation as Loc;
use rand::prelude::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;

/// Wait for input, to slow down overwhelmingly large iterations
pub fn get_input() {
    let mut input_string = String::new();
    std::io::stdin()
        .read_line(&mut input_string)
        .expect("Failed to read line");
}

/// A RNG suitable for testing, if no seed is passed, uses standard random seed.
pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| thread_rng().gen());
    tracing::info!("RNG seed: {}", seed);
    StdRng::seed_from_u64(seed)
}

fn full_len() -> f64 {
    2f64.powf(32.0)
}

#[allow(dead_code)]
type DataVec = statrs::statistics::Data<Vec<f64>>;

/// Your peers just look like a bunch of Arqs
pub type Peers = Vec<Arq>;

/// Construct an arq from start and length specified in the interval [0, 1]
pub fn unit_arq(topo: &Topology, strat: &ArqStrat, unit_start: f64, unit_len: f64) -> Arq {
    assert!(
        (0.0..1.0).contains(&unit_start),
        "center out of bounds {}",
        unit_start
    );
    assert!(
        (0.0..=1.0).contains(&unit_len),
        "len out of bounds {}",
        unit_len
    );

    approximate_arq(
        topo,
        strat,
        Loc::from((unit_start * full_len()) as u32),
        (unit_len * full_len()) as u64,
    )
}

/// Each agent is perfectly evenly spaced around the DHT (+/- some jitter),
/// with stable arc lengths that are sized to meet the minimum coverage target
pub fn generate_ideal_coverage(
    topo: &Topology,
    rng: &mut StdRng,
    strat: &ArqStrat,
    cov: Option<f64>,
    n: u32,
    jitter: f64,
) -> Peers {
    tracing::info!("N = {}, J = {}", n, jitter);
    tracing::info!("ArqStrat: = {:#?}", strat);

    let nf = n as f64;
    // aim for the middle of the coverage target range
    let target = cov.unwrap_or_else(|| strat.midline_coverage());
    let len = (target / nf).min(1.0);

    let peers: Vec<_> = (0..n)
        .map(|i| {
            let center =
                ((i as f64 / nf) + (2.0 * jitter * rng.gen::<f64>()) - jitter).rem_euclid(1.0);
            unit_arq(topo, strat, center, len)
        })
        .collect();

    let cov = actual_coverage(topo, peers.iter());
    // this equation for min is derived from solving for min in terms of buf and target, using
    // the system of equations:
    //     max = min * (buf + 1)
    //     target = (min + max) / 2
    let min = 2.0 * target / (strat.buffer + 2.0);
    let max = min * (strat.buffer + 1.0);
    // When this is perfect, fudge factor can be 1.0. Otherwise, choose a small value >1 like 1.05,
    // to accomodate values slightly out of range.
    let fudge = 1.0;
    let min = (min / fudge).floor();
    let max = (max * fudge).ceil();
    assert!(
        min <= cov && cov <= max,
        "Ideal coverage was generated incorrectly: !({} <= {} <= {})",
        min,
        cov,
        max
    );
    peers
}

/// Generates arqs with lengths according to a standard distribution, to test
/// wildly unbalanced initial conditions
pub fn generate_messy_coverage(
    topo: &Topology,
    rng: &mut StdRng,
    strat: &ArqStrat,
    len_mean: f64,
    len_std: f64,
    n: u32,
    jitter: f64,
) -> Peers {
    use rand::distributions::*;

    tracing::info!("N = {}, J = {}", n, jitter);
    tracing::info!("ArqStrat: = {:#?}", strat);

    let len_dist = statrs::distribution::Normal::new(len_mean, len_std).unwrap();

    let nf = n as f64;

    let peers: Vec<_> = (0..n)
        .map(|i| {
            let center =
                ((i as f64 / nf) + (2.0 * jitter * rng.gen::<f64>()) - jitter).rem_euclid(1.0);
            let len = len_dist.sample(rng).clamp(0.0, 1.0);
            unit_arq(topo, strat, center, len)
        })
        .collect();

    peers
}

#[test]
fn test_unit_arc() {
    let topo = Topology::unit_zero();
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let expected_chunks = 8;

    {
        let a = unit_arq(&topo, &strat, 0.0, 0.0);
        assert_eq!(a.power(), topo.min_space_power());
        assert_eq!(a.count(), 0);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0);
        assert_eq!(a.power(), topo.max_space_power(&strat));
        assert_eq!(a.count(), 8);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0 / 2.0);
        assert_eq!(a.power(), 28);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0 / 4.0);
        assert_eq!(a.power(), 27);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0 / 8.0);
        assert_eq!(a.power(), 26);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0 / 16.0);
        assert_eq!(a.power(), 25);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&topo, &strat, 0.0, 1.0 / 32.0);
        assert_eq!(a.power(), 24);
        assert_eq!(a.count(), expected_chunks);
    }
}

#[cfg(test)]
mod tests {
    use crate::arq::PeerViewQ;

    use super::*;
    use proptest::proptest;
    use test_case::test_case;

    #[test_case(21.62, 0.1, 100)]
    #[test_case(89.6, 0.25, 100)]
    fn test_ideal_coverage_cases(min_coverage: f64, buffer: f64, num_peers: u32) {
        let topo = Topology::unit_zero();

        let strat = ArqStrat {
            min_coverage,
            buffer,
            ..Default::default()
        };

        let mut rng = seeded_rng(None);
        let peers = generate_ideal_coverage(&topo, &mut rng, &strat, None, num_peers, 0.0);

        let view = PeerViewQ::new(topo, strat.clone(), peers);
        let cov = view.actual_coverage();

        let min = strat.min_coverage;
        let max = strat.max_coverage();
        assert!(min <= cov);
        assert!(cov <= max);
    }

    proptest! {
        /// Ensure that something close to the ideal coverage is generated under a
        /// range of ArqStrat parameters.
        /// NOTE: this is not perfect. The final assertion has to be fudged a bit,
        /// so this test asserts that the extrapolated coverage falls within the
        /// range.
        #[test]
        fn test_ideal_coverage(min_coverage in 40f64..100.0, buffer in 0.1f64..0.25, num_peers in 100u32..200) {
            let topo = Topology::unit_zero();
            let strat = ArqStrat {
                min_coverage,
                buffer,
                ..Default::default()
            };
            let mut rng = seeded_rng(None);
            let peers = generate_ideal_coverage(&topo, &mut rng, &strat, None, num_peers, 0.0);
            let view = PeerViewQ::new(topo, strat.clone(), peers);
            let cov = view.actual_coverage();

            let min = strat.min_coverage;
            let max = strat.max_coverage();
            assert!(min <= cov, "extrapolated less than min {} <= {}", min, cov);
            assert!(cov <= max, "extrapolated greater than max {} <= {}", cov, max);
        }

        #[test]
        fn chunk_count_is_always_within_bounds(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let topo = Topology::unit_zero();
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&topo, &strat, center, len);

            assert!(a.count() >= strat.min_chunks());
            assert!(a.count() <= strat.max_chunks());
        }

        #[test]
        fn power_is_always_within_bounds(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let topo = Topology::unit_zero();
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&topo, &strat, center, len);
            assert!(a.power() >= topo.min_space_power());
            assert!(a.power() <= topo.max_space_power(&strat));
        }

        #[test]
        fn length_is_always_close(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let topo = Topology::unit_zero();
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&topo, &strat, center, len);
            let target_len = (len * 2f64.powf(32.0)) as i64;
            let true_len = a.to_dht_arc_range(&topo).length() as i64;
            assert!((true_len - target_len).abs() < a.absolute_chunk_width(&topo) as i64);
        }
    }
}
