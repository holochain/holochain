use kitsune_p2p_dht::arq::Arq;
use kitsune_p2p_dht::arq::ArqStrat;
use kitsune_p2p_dht_arc::*;
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
    let center = DhtLocation::from((full_len * center) as u32);
    if len == 1.0 {
        Arq::new_full(center, strat.max_power)
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
            let len = coverage / nf;
            unit_arq(strat, center, len, power_offset)
        })
        .collect()
}

/// View ascii for all arcs
pub fn print_arqs(arqs: &Peers) {
    for (i, arq) in arqs.into_iter().enumerate() {
        println!("|{}| {}", arq.to_interval().to_ascii(64), i);
    }
}
