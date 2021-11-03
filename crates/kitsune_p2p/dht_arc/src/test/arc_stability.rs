use crate::PeerStrat;
use crate::*;
use pretty_assertions::assert_eq;
use rand::thread_rng;
use rand::Rng;
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

type Peers = Vec<DhtArc>;

fn full_len() -> f64 {
    2f64.powi(32)
}

#[test]
fn only_change_one() {
    std::env::set_var("RUST_LOG", "info");
    observability::test_run().ok();
    use Vergence::*;

    let redundancy = 100;

    let run = |iters, n, j, check_gaps| {
        tracing::info!("");
        tracing::info!("------------------------");
        let strat = PeerStratAlpha {
            check_gaps,
            redundancy_target: redundancy / 2,
            ..Default::default()
        }
        .into();

        let s = ArcLenStrategy::Constant(redundancy as f64 / n as f64);
        let mut peers = simple_parameterized_generator(n, j, s);
        peers[0].half_length = MAX_HALF_LENGTH;
        let equilibrium = determine_equilibrium(iters, peers, |peers| {
            let dynamic = Some(maplit::hashset![0]);
            let (peers, stats) = run_one_epoch(&strat, peers, dynamic.as_ref(), DETAIL);
            tracing::debug!("{}", peers[0].coverage());
            (peers, stats)
        });
        // print_arcs(&peers);
        report(&equilibrium);
        equilibrium
    };

    // These diverge only rarely
    let _borderline = vec![
        run(16, 1000, 0.001, false).vergence(),
        run(16, 1000, 0.01, false).vergence(),
    ];

    let divergent = vec![
        run(8, 1000, 0.0, true).vergence(),
        run(8, 1000, 0.0003, true).vergence(),
    ];

    let convergent = vec![
        run(16, 1000, 0.0, false).vergence(),
        run(16, 1000, 0.0003, false).vergence(),
        run(16, 1000, 0.0007, false).vergence(),
    ];

    // assert_eq!(borderline, vec![Divergent; borderline.len()]);
    assert_eq!(divergent, vec![Divergent; divergent.len()]);
    assert_eq!(convergent, vec![Convergent; convergent.len()]);

    // assert!(matches!(run(true), Divergent(_)));
    // assert!(matches!(run(false), Convergent(_)));
}

#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "info");
    observability::test_run().ok();
    let n = 1000;
    let j = 1f64 / n as f64 / 3.0;
    let s = ArcLenStrategy::Constant(0.1);

    let r = 50;
    let strat = PeerStratAlpha {
        redundancy_target: r,
        ..Default::default()
    }
    .into();

    let peers = simple_parameterized_generator(n, j, s);
    tracing::info!("");
    tracing::info!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(8, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, DETAIL);
        tracing::info!("{}", stats.oneline());
        (peers, stats)
    });
    report(&eq);
    eq.assert_convergent();
}

#[test]
fn min_redundancy_is_maintained() {
    todo!("Check that min redundancy is maintained at all times");
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
        let mut peers_clone = peers.clone();
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
    Run { vergence, history }
}

/// Resize every arc based on neighbors' arcs, and compute stats about this iteration
/// kind: The resizing strategy to use
/// peers: The list of peers in this epoch
/// dynamic_peer_indices: Indices of peers who should be updated. If None, all peers will be updated.
/// detail: Level of output detail. More is more verbose. detail: u8,
fn run_one_epoch(
    kind: &PeerStrat,
    mut peers: Peers,
    dynamic_peer_indices: Option<&HashSet<usize>>,
    detail: u8,
) -> (Peers, EpochStats) {
    let mut net = 0.0;
    let mut gross = 0.0;
    let mut delta_min = full_len() / 2.0;
    let mut delta_max = -full_len() / 2.0;
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
        let bucket = DhtArcBucket::new(*arc, p.clone());
        let density = bucket.peer_view(kind);
        let before = arc.absolute_length() as f64;
        arc.update_length(density);
        let after = arc.absolute_length() as f64;
        let delta = after - before;
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
        net_delta_avg: net / tot / full_len(),
        gross_delta_avg: gross / tot / full_len(),
        min_redundancy: min_redundancy,
        delta_min: delta_min / full_len(),
        delta_max: delta_max / full_len(),
    };
    (peers, stats)
}

/// Generate a list of DhtArcs based on 3 parameters:
/// N: total # of peers
/// J: random jitter of peer locations
/// S: strategy for generating arc lengths
fn simple_parameterized_generator(n: usize, j: f64, s: ArcLenStrategy) -> Peers {
    tracing::info!("N = {}, J = {}", n, j);
    tracing::info!("Arc len generation: {:?}", s);
    let halflens = s.gen(n);
    generate_evenly_spaced_with_half_lens_and_jitter(j, halflens)
}

/// Define arcs by centerpoint and halflen in the unit interval [0.0, 1.0]
fn unit_arcs<H: Iterator<Item = (f64, f64)>>(arcs: H) -> Peers {
    let fc = full_len();
    let fh = MAX_HALF_LENGTH as f64;
    arcs.map(|(c, h)| DhtArc::new((c * fc).min(u32::MAX as f64) as u32, (h * fh) as u32))
        .collect()
}

/// Each agent is perfect evenly spaced around the DHT,
/// with the halflens specified by the iterator.
fn generate_evenly_spaced_with_half_lens_and_jitter<H: Iterator<Item = f64>>(
    jitter: f64,
    hs: H,
) -> Peers {
    let mut rng = thread_rng();
    let hs: Vec<_> = hs.collect();
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
enum ArcLenStrategy {
    Random,
    Constant(f64),
    HalfAndHalf(f64, f64),
}

impl ArcLenStrategy {
    pub fn gen(&self, num: usize) -> Box<dyn Iterator<Item = f64>> {
        match self {
            Self::Random => {
                let mut rng = thread_rng();
                Box::new(iter::repeat_with(move || rng.gen()).take(num))
            }
            Self::Constant(v) => Box::new(iter::repeat(*v).take(num)),
            Self::HalfAndHalf(a, b) => Box::new(
                iter::repeat(*a)
                    .take(num / 2)
                    .chain(iter::repeat(*b).take(num / 2)),
            ),
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
