use kitsune_p2p_dht::arq::Arq;
use kitsune_p2p_dht::arq::ArqSet;
use kitsune_p2p_dht::arq::ArqStrat;
use kitsune_p2p_dht::arq::PeerView;
use kitsune_p2p_dht_arc::DhtLocation as Loc;
use rand::prelude::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;

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

pub fn unit_arq(strat: &ArqStrat, center: f64, len: f64, power_offset: i8) -> Arq {
    assert!(
        0.0 <= center && center < 1.0,
        "center out of bounds {}",
        center
    );
    assert!(0.0 <= len && len <= 1.0, "len out of bounds {}", len);

    let full_len = full_len();
    let center = Loc::from((full_len * center) as u32);
    if len == 1.0 {
        Arq::new_full(center, strat.max_power)
    } else if len == 0.0 {
        Arq::new(center, strat.min_power, 0)
    } else {
        let po = power_offset as f64;

        // the log2 of the total length gives us a real number power,
        // where the integral part is the power such that the
        // length is between 1 and 2 chunks long at that power,
        // and the fractional part tells us what fraction of a chunk would give
        // us this length
        let log_len = (len * full_len).log2();
        let log_len_rem = log_len.rem_euclid(1.0);

        // log2 of min_chunks lets us know by how much to reduce the above power
        // so that we have a small enough power to actually use at least min_chunks
        // number of chunks. For instance if min_chunks is 8, this power is 3,
        // which will be subtracted from the above power to allow for
        // at least 8 chunks in representing the length.
        let pow_min_chunks = (strat.min_chunks() as f64).log2();

        let power = (log_len + po) as u32 - pow_min_chunks as u32;

        let count = ((1.0 + log_len_rem) * strat.min_chunks() as f64) as u32;
        // dbg!(power, pow_min_chunks, log_len_rem, count);
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
    n: u32,
    jitter: f64,
    power_offset: i8,
) -> Peers {
    tracing::info!("N = {}, J = {}", n, jitter);
    tracing::info!("ArqStrat: = {:#?}", strat);

    let nf = n as f64;
    let coverage = strat.min_coverage as f64;

    (0..n)
        .map(|i| {
            let center =
                ((i as f64 / nf) + (2.0 * jitter * rng.gen::<f64>()) - jitter).rem_euclid(1.0);
            let len = (coverage / nf).min(1.0);
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
    let expected_chunks = strat.min_chunks();

    let a1 = unit_arq(&strat, 0.0, 0.0, 0);
    assert_eq!(a1.power(), strat.min_power);
    assert_eq!(a1.count(), 0);

    let a1 = unit_arq(&strat, 0.0, 1.0, 0);
    assert_eq!(a1.power(), 29);
    assert_eq!(a1.count(), expected_chunks);

    let a2 = unit_arq(&strat, 0.0, 1.0 / 2.0, 0);
    assert_eq!(a2.power(), 29);
    assert_eq!(a2.count(), expected_chunks);

    let a3 = unit_arq(&strat, 0.0, 1.0 / 4.0, 0);
    assert_eq!(a3.power(), 28);
    assert_eq!(a3.count(), expected_chunks);

    let a4 = unit_arq(&strat, 0.0, 1.0 / 8.0, 0);
    assert_eq!(a4.power(), 27);
    assert_eq!(a4.count(), expected_chunks);

    let a5 = unit_arq(&strat, 0.0, 1.0 / 16.0, 0);
    assert_eq!(a5.power(), 26);
    assert_eq!(a5.count(), expected_chunks);

    let a6 = unit_arq(&strat, 0.0, 1.0 / 32.0, 0);
    assert_eq!(a6.power(), 25);
    assert_eq!(a6.count(), expected_chunks);
}

/// View ascii for all arcs
pub fn print_arqs(arqs: &ArqSet, len: usize) {
    println!("{} arqs, power: {}", arqs.arqs().len(), arqs.power());
    for (i, arq) in arqs.arqs().into_iter().enumerate() {
        println!(
            "|{}| {}:\t{}",
            arq.to_interval().to_ascii(len),
            i,
            arq.count()
        );
    }
}

use proptest::proptest;

#[test]
fn test_ideal_coverage_case() {
    let strat = ArqStrat {
        min_coverage: 10.0,
        buffer: 0.144,
        ..Default::default()
    };

    let mut rng = seeded_rng(None);
    let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
    let peer_arqs = generate_ideal_coverage(&mut rng, &strat, 100, 0.0, 0);

    let peers = ArqSet::new(peer_arqs.into_iter().map(|arq| arq.to_bounds()).collect());

    let view = PeerView::new(strat.clone(), peers);
    let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
    assert!((dbg!(extrapolated) - strat.min_coverage).abs() < strat.buffer_width());
}

proptest! {
    #[test]
    fn test_ideal_coverage(min_coverage in 20f64..50.0, buffer in 0.1f64..0.5) {
        let strat = ArqStrat {
            min_coverage,
            buffer,
            ..Default::default()
        };
        let mut rng = seeded_rng(None);
        let arq = Arq::new_full(Loc::from(0x0), strat.max_power);
        let peer_arqs = generate_ideal_coverage(&mut rng, &strat, 100, 0.0, 0);

        let peers = ArqSet::new(peer_arqs.into_iter().map(|arq| arq.to_bounds()).collect());
        print_arqs(&peers, 64);

        let view = PeerView::new(strat.clone(), peers);
        let extrapolated = view.extrapolated_coverage(&arq.to_bounds());
        println!(
            "{} <= {} <= {}",
            strat.min_coverage,
            extrapolated,
            strat.max_coverage()
        );
        assert!(strat.min_coverage <= extrapolated);
        assert!(extrapolated <= strat.max_coverage());
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
