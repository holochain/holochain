pub mod gossip_direct;
pub mod op_store;
pub mod test_node;

use crate::arq::Arq;
use crate::arq::ArqStrat;

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
        .ok()
        .expect("Failed to read line");
}

pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| thread_rng().gen());
    tracing::info!("RNG seed: {}", seed);
    StdRng::seed_from_u64(seed)
}

fn full_len() -> f64 {
    2f64.powf(32.0)
}

type DataVec = statrs::statistics::Data<Vec<f64>>;

pub type Peers = Vec<Arq>;

pub fn unit_arq(strat: &ArqStrat, unit_center: f64, unit_len: f64, power_offset: i8) -> Arq {
    assert!(
        0.0 <= unit_center && unit_center < 1.0,
        "center out of bounds {}",
        unit_center
    );
    assert!(
        0.0 <= unit_len && unit_len <= 1.0,
        "len out of bounds {}",
        unit_len
    );

    if power_offset != 0 {
        unimplemented!(
            "power_offset logic is not yet tested, and should not be used before revisiting"
        );
    }

    let full_len = full_len();
    let center = Loc::from((full_len * unit_center) as u32);
    if unit_len == 1.0 {
        Arq::new_full(center, strat.max_power)
    } else if unit_len == 0.0 {
        Arq::new(center, strat.min_power, 0)
    } else {
        let po = power_offset as f64;
        let len = unit_len * full_len;

        // the log2 of the length tells us roughly what `power` to use to
        // represent the entire length as a single chunk.
        let log_len = len.log2();

        // log2 of min_chunks lets us know by how much to reduce the above power
        // so that we have a small enough power to actually use at least min_chunks
        // number of chunks. For instance if min_chunks is 8, this power is 3,
        // which will be subtracted from the above power to allow for
        // at least 8 chunks in representing the length.
        let pow_min_chunks = (strat.min_chunks() as f64).log2();

        // Find the difference as described above, including the power_offset.
        // NOTE: this should be modified to take the ArqStrat::buffer into
        // consideration, because a narrower buffer requires a smaller power.
        let power = (log_len + po - pow_min_chunks).floor();

        let q = 2f64.powf(power);
        let count = (len / q).round() as u32;

        let min = strat.min_chunks() as f64 * 2f64.powf(po);
        let max = strat.max_chunks() as f64 * 2f64.powf(po);
        assert!(count >= min as u32, "count < min: {} < {}", count, min);
        assert!(count <= max as u32, "count > max: {} > {}", count, max);
        Arq::new(center, power as u8, count)
    }
}

/// Each agent is perfectly evenly spaced around the DHT (+/- some jitter),
/// with stable arc lengths that are sized to meet the minimum coverage target
pub fn generate_ideal_coverage(
    rng: &mut StdRng,
    strat: &ArqStrat,
    cov: Option<f64>,
    n: u32,
    jitter: f64,
    power_offset: i8,
) -> Peers {
    tracing::info!("N = {}, J = {}", n, jitter);
    tracing::info!("ArqStrat: = {:#?}", strat);

    let nf = n as f64;
    // aim for the middle of the coverage target range
    let coverage = cov.unwrap_or_else(|| (strat.min_coverage + strat.max_coverage()) / 2.0);
    let len = (coverage / nf).min(1.0);

    (0..n)
        .map(|i| {
            let center =
                ((i as f64 / nf) + (2.0 * jitter * rng.gen::<f64>()) - jitter).rem_euclid(1.0);

            unit_arq(strat, center, len, power_offset)
        })
        .collect()
}

#[test]
fn test_unit_arc() {
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.2,
        ..Default::default()
    };
    let expected_chunks = 8;

    {
        let a = unit_arq(&strat, 0.0, 0.0, 0);
        assert_eq!(a.power(), strat.min_power);
        assert_eq!(a.count(), 0);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0, 0);
        assert_eq!(a.power(), 29);
        assert_eq!(a.count(), 8);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0 / 2.0, 0);
        assert_eq!(a.power(), 28);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0 / 4.0, 0);
        assert_eq!(a.power(), 27);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0 / 8.0, 0);
        assert_eq!(a.power(), 26);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0 / 16.0, 0);
        assert_eq!(a.power(), 25);
        assert_eq!(a.count(), expected_chunks);
    }
    {
        let a = unit_arq(&strat, 0.0, 1.0 / 32.0, 0);
        assert_eq!(a.power(), 24);
        assert_eq!(a.count(), expected_chunks);
    }
}

#[cfg(test)]
mod tests {
    use crate::arq::{ArqSet, PeerView};

    use super::*;
    use proptest::proptest;

    #[test]
    fn test_ideal_coverage_case() {
        let strat = ArqStrat {
            // min_coverage: 44.93690369578987,
            // buffer: 0.1749926,
            min_coverage: 21.620980,
            buffer: 0.1,
            ..Default::default()
        };

        let mut rng = seeded_rng(None);
        let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        let peer_arqs = generate_ideal_coverage(&mut rng, &strat, None, 100, 0.0, 0);

        let peers = ArqSet::new(peer_arqs.into_iter().map(|arq| arq.to_bounds()).collect());

        let view = PeerView::new(strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq.to_bounds());

        // TODO: tighten this up so we don't need +/- 1
        let min = strat.min_coverage - 1.0;
        let max = strat.max_coverage() + 1.0;
        println!("{} <= {} <= {}", min, extrapolated, max);
        assert!(min <= extrapolated);
        assert!(extrapolated <= max);
    }

    proptest! {
        /// Ensure that something close to the ideal coverage is generated under a
        /// range of ArqStrat parameters.
        /// NOTE: this is not perfect. The final assertion has to be fudged a bit,
        /// so this test asserts that the extrapolated coverage falls within the
        /// range, +/- 1 on either end.
        #[test]
        fn test_ideal_coverage(min_coverage in 40f64..100.0, buffer in 0.1f64..0.5) {
            let strat = ArqStrat {
                min_coverage,
                buffer,
                ..Default::default()
            };
            let mut rng = seeded_rng(None);
            let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
            let peer_arqs = generate_ideal_coverage(&mut rng, &strat, None, 100, 0.0, 0);

            let peers = ArqSet::new(peer_arqs.into_iter().map(|arq| arq.to_bounds()).collect());

            let view = PeerView::new(strat.clone(), peers);
            let extrapolated = view.extrapolated_coverage(&arq.to_bounds());

            // TODO: tighten this up so we don't need +/- 1
            let min = strat.min_coverage - 1.0;
            let max = strat.max_coverage() + 1.0;
            assert!(min <= extrapolated, "extrapolated less than min {} <= {}", min, extrapolated);
            assert!(extrapolated <= max, "extrapolated greater than max {} <= {}", extrapolated, max);
        }

        #[test]
        fn chunk_count_is_always_within_bounds(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&strat, center, len, 0);
            // println!(
            //     "{} <= {} <= {}",
            //     strat.min_chunks(),
            //     a.count(),
            //     strat.max_chunks()
            // );
            assert!(a.count() >= strat.min_chunks());
            assert!(a.count() <= strat.max_chunks());
        }

        #[test]
        fn power_is_always_within_bounds(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&strat, center, len, 0);
            println!(
                "{} <= {} <= {}",
                strat.min_power,
                a.power(),
                strat.max_power
            );
            assert!(a.power() >= strat.min_power);
            assert!(a.power() <= strat.max_power);
        }

        #[test]
        fn length_is_always_close(center in 0.0f64..0.999, len in 0.001f64..1.0) {
            let strat = ArqStrat {
                min_coverage: 10.0,
                buffer: 0.144,
                ..Default::default()
            };
            let a = unit_arq(&strat, center, len, 0);
            let target_len = (len * 2f64.powf(32.0)) as i64;
            let true_len = a.to_interval().length() as i64;
            println!("{} ~ {} ({})", true_len, target_len, a.spacing());
            assert!((true_len - target_len).abs() < a.spacing() as i64);
        }
    }
}
