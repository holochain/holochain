use crate::PeerStrat;
use crate::*;
use pretty_assertions::assert_eq;
use rand::prelude::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;
use statrs::statistics::*;
use std::collections::HashSet;
use std::iter;

/// Maximum number of iterations. If we iterate this much, we assume the
/// system is divergent (unable to reach equilibrium).
const DIVERGENCE_ITERS: usize = 64;

/// If a system converges this many times, consider it convergent.
/// If it diverges once, consider it divergent.
/// Increase this number if tests become flaky, decrease it if tests are too slow.
const DETERMINATION_ITERS: usize = 8;

/// Number of consecutive rounds of no movement before declaring convergence.
const CONVERGENCE_WINDOW: usize = 3;

/// Level of detail in reporting.
const DETAIL: u8 = 0;

type DataVec = statrs::statistics::Data<Vec<f64>>;

pub type Peers = Vec<DhtArc>;

fn max_halflen() -> f64 {
    MAX_HALF_LENGTH as f64
}

fn full_len() -> f64 {
    2f64.powi(32)
}

pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| thread_rng().gen());
    tracing::info!("RNG seed: {}", seed);
    StdRng::seed_from_u64(seed)
}

#[test]
fn single_agent_convergence_debug() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let n = 50;
    let j = 0.1;
    let redundancy = 5;
    let check_gaps = false;

    let mut rng = seeded_rng(None);
    // let mut rng = seeded_rng(Some(5181023930453438019));

    let strat = PeerStratAlpha {
        check_gaps,
        redundancy_target: redundancy / 2,
        ..Default::default()
    }
    .into();

    let s = ArcLenStrategy::Constant(redundancy as f64 / n as f64);

    let mut peers = simple_parameterized_generator(&mut rng, n, j, s);
    peers[0].half_length = MAX_HALF_LENGTH;
    tracing::debug!("{}", EpochStats::oneline_header());
    let runs = determine_equilibrium(1, peers, |peers| {
        let dynamic = Some(maplit::hashset![0]);
        let (peers, stats) = run_one_epoch(&strat, peers, dynamic.as_ref(), DETAIL);

        tracing::debug!("{}", stats.oneline());
        // tracing::debug!("{}", peers[0].coverage());
        (peers, stats)
    });
    print_arcs(&runs.0[0].peers);
    report(&runs);
}

#[allow(dead_code)]
fn run_single_agent_convergence(
    iters: usize,
    n: usize,
    redundancy: u16,
    j: f64,
    check_gaps: bool,
) -> RunBatch {
    tracing::info!("");
    tracing::info!("------------------------");

    // let seed = None;
    let seed = Some(7532095396949412554);
    let mut rng = seeded_rng(seed);

    let strat = PeerStratAlpha {
        check_gaps,
        redundancy_target: redundancy / 2,
        ..Default::default()
    }
    .into();

    let s = ArcLenStrategy::Constant(redundancy as f64 / n as f64);

    let mut peers = simple_parameterized_generator(&mut rng, n, j, s);
    peers[0].half_length = MAX_HALF_LENGTH;
    let runs = determine_equilibrium(iters, peers, |peers| {
        let dynamic = Some(maplit::hashset![0]);
        let (peers, stats) = run_one_epoch(&strat, peers, dynamic.as_ref(), DETAIL);
        tracing::debug!("{}", peers[0].coverage());
        (peers, stats)
    });
    report(&runs);
    runs
}

/// Test if various distributions of agents can converge
#[test]
#[cfg(feature = "slow_tests")]
fn single_agent_convergence_battery() {
    std::env::set_var("RUST_LOG", "info");
    observability::test_run().ok();
    use Vergence::*;

    let n = 1000;
    let r = 100;

    let divergent = vec![
        run_single_agent_convergence(8, n, r, 0.1, true).vergence(),
        run_single_agent_convergence(8, n, r, 0.5, true).vergence(),
        run_single_agent_convergence(8, n, r, 1.0, true).vergence(),
    ];

    let convergent = vec![
        // gap_check == true
        run_single_agent_convergence(8, n, r, 0.0, true).vergence(),
        run_single_agent_convergence(8, n, r, 0.001, true).vergence(),
        run_single_agent_convergence(8, n, r, 0.01, true).vergence(),
        // gap_check == false
        run_single_agent_convergence(8, n, r, 0.0, false).vergence(),
        run_single_agent_convergence(8, n, r, 0.001, false).vergence(),
        run_single_agent_convergence(8, n, r, 0.01, false).vergence(),
        // Note that these same scenarios fail to converge with gap_check
        run_single_agent_convergence(8, n, r, 0.1, false).vergence(),
        run_single_agent_convergence(8, n, r, 0.5, false).vergence(),
        run_single_agent_convergence(8, n, r, 1.0, false).vergence(),
    ];

    assert_eq!(divergent, vec![Divergent; divergent.len()]);
    assert_eq!(convergent, vec![Convergent; convergent.len()]);
}

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 1000;
    let j = 10.0 / n as f64;
    let s = ArcLenStrategy::Constant(0.1);

    let r = 50;
    let strat = PeerStratAlpha {
        redundancy_target: r,
        ..Default::default()
    }
    .into();

    let peers = simple_parameterized_generator(&mut rng, n, j, s);
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(2, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, DETAIL);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    report(&eq);
    eq.assert_convergent();
    // TODO: the min redundancy is never exactly 100.
    //       would be good to look at the *average* redundancy, and other stats.
    eq.assert_min_redundancy(96);
}

fn report(e: &RunBatch) {
    let counts = DataVec::new(e.histories().map(|h| h.len() as f64).collect());

    if let Vergence::Convergent = e.vergence() {
        tracing::info!(
            "Reached equilibrium in {} mean iterations (variance {})",
            counts.mean().unwrap(),
            counts.variance().unwrap()
        );
    } else {
        tracing::warn!(
            "Divergent run found on attempt #{}. Failed to reach equilibrium in {} iterations",
            e.histories().count(),
            DIVERGENCE_ITERS
        );
    }
}

fn determine_equilibrium<'a, F>(iters: usize, peers: Peers, step: F) -> RunBatch
where
    F: 'a + Clone + Fn(Peers) -> (Peers, EpochStats),
{
    use Vergence::*;
    let mut runs = vec![];
    for i in 1..=iters {
        tracing::debug!("----- Running equilibrium iteration {} -----", i);
        let run = seek_convergence(peers.clone(), |peers| step(peers));
        let vergence = run.vergence;
        runs.push(run);
        if vergence == Divergent {
            break;
        }
    }
    RunBatch(runs)
}

/// Run iterations until there is no movement of any arc
/// TODO: this may be unreasonable, and we may need to just ensure that arcs
/// settle down into a reasonable level of oscillation
fn seek_convergence<'a, F>(peers: Peers, step: F) -> Run
where
    F: Fn(Peers) -> (Peers, EpochStats),
{
    let converged = |convergence| convergence >= CONVERGENCE_WINDOW;
    let (peers, history, convergence) = (1..=DIVERGENCE_ITERS).fold(
        (peers, vec![], 0),
        |(peers, mut history, mut convergence), _i| {
            if !converged(convergence) {
                let (peers, stats) = step(peers);
                if stats.gross_delta_avg == 0.0 {
                    convergence += 1;
                } else if convergence > 0 {
                    panic!(
                        "we don't expect a system in equilibirum to suddenly start moving again."
                    )
                } else {
                    history.push(stats);
                }
                (peers, history, convergence)
            } else {
                (peers, history, convergence)
            }
        },
    );

    let vergence = if converged(convergence) {
        Vergence::Convergent
    } else {
        Vergence::Divergent
    };
    Run {
        vergence,
        history,
        peers,
    }
}

/// Resize every arc based on neighbors' arcs, and compute stats about this iteration
/// strat: The resizing strategy to use
/// peers: The list of peers in this epoch
/// dynamic_peer_indices: Indices of peers who should be updated. If None, all peers will be updated.
/// detail: Level of output detail. More is more verbose. detail: u8,
fn run_one_epoch(
    strat: &PeerStrat,
    mut peers: Peers,
    dynamic_peer_indices: Option<&HashSet<usize>>,
    detail: u8,
) -> (Peers, EpochStats) {
    let mut net = 0.0;
    let mut gross = 0.0;
    let mut delta_min = max_halflen();
    let mut delta_max = -max_halflen();
    let mut index_min = peers.len();
    let mut index_max = peers.len();
    for i in 0..peers.len() {
        if let Some(dynamic) = dynamic_peer_indices {
            if !dynamic.contains(&i) {
                continue;
            }
        }
        let p = peers.clone();
        let arc = peers.get_mut(i).unwrap();
        let view = strat.view(*arc, p.as_slice());
        let before = arc.half_length() as f64;
        arc.update_length(view);
        let after = arc.half_length() as f64;
        let delta = after - before;
        // dbg!(&before, &after, &delta);
        net += delta;
        gross += delta.abs();
        if delta < delta_min {
            delta_min = delta;
            index_min = i;
        }
        if delta > delta_max {
            delta_max = delta;
            index_max = i;
        }
    }

    if detail == 1 {
        tracing::info!("min: |{}| {}", peers[index_min].to_ascii(64), index_min);
        tracing::info!("max: |{}| {}", peers[index_max].to_ascii(64), index_max);
        tracing::info!("");
    } else if detail == 2 {
        print_arcs(&peers);
        get_input();
    }

    let tot = peers.len() as f64;
    let min_redundancy = check_redundancy(peers.clone());
    let stats = EpochStats {
        net_delta_avg: net / tot / max_halflen(),
        gross_delta_avg: gross / tot / max_halflen(),
        min_redundancy: min_redundancy,
        delta_min: delta_min / max_halflen(),
        delta_max: delta_max / max_halflen(),
    };
    (peers, stats)
}

/// Generate a list of DhtArcs based on 3 parameters:
/// N: total # of peers
/// J: random jitter of peer locations
/// S: strategy for generating arc lengths
pub fn simple_parameterized_generator(
    rng: &mut StdRng,
    n: usize,
    j: f64,
    s: ArcLenStrategy,
) -> Peers {
    tracing::info!("N = {}, J = {}", n, j);
    tracing::info!("Arc len generation: {:?}", s);
    let halflens = s.gen(rng, n);
    generate_evenly_spaced_with_half_lens_and_jitter(rng, j, halflens)
}

/// Define arcs by centerpoint and halflen in the unit interval [0.0, 1.0]
pub fn unit_arcs<H: Iterator<Item = (f64, f64)>>(arcs: H) -> Peers {
    let fc = full_len();
    let fh = MAX_HALF_LENGTH as f64;
    arcs.map(|(c, h)| DhtArc::new((c * fc).min(u32::MAX as f64) as u32, (h * fh) as u32))
        .collect()
}

/// Each agent is perfect evenly spaced around the DHT,
/// with the halflens specified by the iterator.
pub fn generate_evenly_spaced_with_half_lens_and_jitter(
    rng: &mut StdRng,
    jitter: f64,
    hs: Vec<f64>,
) -> Peers {
    let n = hs.len() as f64;
    unit_arcs(hs.into_iter().enumerate().map(|(i, h)| {
        (
            (i as f64 / n) + (2.0 * jitter * rng.gen::<f64>()) - jitter,
            h,
        )
    }))
}

#[derive(Debug)]
struct RunBatch(Vec<Run>);

impl RunBatch {
    pub fn vergence(&self) -> Vergence {
        if self.0.iter().all(|r| r.vergence == Vergence::Convergent) {
            Vergence::Convergent
        } else {
            Vergence::Divergent
        }
    }

    pub fn histories(&self) -> impl Iterator<Item = &Vec<EpochStats>> + '_ {
        self.0.iter().map(|r| &r.history)
    }

    pub fn assert_convergent(&self) {
        assert_eq!(
            self.vergence(),
            Vergence::Convergent,
            "failed to reach equilibrium in {} iterations",
            DIVERGENCE_ITERS
        )
    }

    pub fn assert_min_redundancy(&self, r: u32) {
        assert!(
            self.histories().flatten().all(|s| s.min_redundancy >= r),
            "redundancy fell below {}",
            r
        )
    }

    pub fn assert_divergent(&self) {
        assert_eq!(
            self.vergence(),
            Vergence::Divergent,
            "sequence was expected to diverge, but converged",
        )
    }
}

#[derive(Debug)]
struct Run {
    vergence: Vergence,
    history: Vec<EpochStats>,
    /// the final state of the peers at the last iteration
    peers: Peers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vergence {
    Convergent,
    Divergent,
}

#[derive(Debug)]
struct EpochStats {
    net_delta_avg: f64,
    gross_delta_avg: f64,
    delta_max: f64,
    delta_min: f64,
    // delta_variance: f64,
    min_redundancy: u32,
}

impl EpochStats {
    pub fn oneline_header() -> String {
        format!("rdun   net Δ%   gross Δ%   min Δ%   max Δ%")
    }

    pub fn oneline(&self) -> String {
        format!(
            "{:4}   {:>+6.3}   {:>8.3}   {:>6.3}   {:>6.3}",
            self.min_redundancy,
            self.net_delta_avg * 100.0,
            self.gross_delta_avg * 100.0,
            self.delta_min * 100.0,
            self.delta_max * 100.0,
        )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ArcLenStrategy {
    Random,
    Constant(f64),
    HalfAndHalf(f64, f64),
}

impl ArcLenStrategy {
    pub fn gen(&self, rng: &mut StdRng, num: usize) -> Vec<f64> {
        match self {
            Self::Random => iter::repeat_with(|| rng.gen()).take(num).collect(),
            Self::Constant(v) => iter::repeat(*v).take(num).collect(),
            Self::HalfAndHalf(a, b) => iter::repeat(*a)
                .take(num / 2)
                .chain(iter::repeat(*b).take(num / 2))
                .collect(),
        }
    }
}

/// View ascii for all arcs
fn print_arcs(arcs: &Peers) {
    for (i, arc) in arcs.into_iter().enumerate() {
        println!("|{}| {}", arc.to_ascii(64), i);
    }
}

/// Wait for input, to slow down overwhelmingly large iterations
fn get_input() {
    let mut input_string = String::new();
    std::io::stdin()
        .read_line(&mut input_string)
        .ok()
        .expect("Failed to read line");
}
