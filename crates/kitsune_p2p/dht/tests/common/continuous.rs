#![cfg(feature = "testing")]

use kitsune_p2p_dht::test_utils::get_input;
use kitsune_p2p_dht_arc::*;
use rand::prelude::StdRng;
use rand::thread_rng;
use rand::Rng;
use rand::SeedableRng;
use statrs::statistics::*;
use std::collections::HashSet;
use std::iter;

/// Maximum number of iterations. If we iterate this much, we assume the
/// system is divergent (unable to reach equilibrium).
const DIVERGENCE_ITERS: usize = 40;

/// Number of consecutive rounds of no movement before declaring convergence.
const CONVERGENCE_WINDOW: usize = 3;

/// Level of detail in reporting.
pub const DETAIL: u8 = 1;

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

pub fn determine_equilibrium<'a, F>(iters: usize, peers: Peers, step: F) -> RunBatch
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
pub fn seek_convergence<'a, F>(peers: Peers, step: F) -> Run
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
pub fn run_one_epoch(
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

    if detail >= 2 {
        tracing::info!("min: |{}| {}", peers[index_min].to_ascii(64), index_min);
        tracing::info!("max: |{}| {}", peers[index_max].to_ascii(64), index_max);
        tracing::info!("");
    } else if detail >= 3 {
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
pub struct RunBatch(Vec<Run>);

#[derive(Clone, Debug)]
pub struct Stats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub variance: f64,
}

impl Stats {
    pub fn new(xs: DataVec) -> Self {
        Self {
            min: xs.min(),
            max: xs.max(),
            mean: xs.mean().unwrap(),
            median: xs.median(),
            variance: xs.variance().unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub iteration_stats: Stats,
    pub overall_redundancy_stats: Stats,
    pub outcome: RunReportOutcome,
    pub total_runs: usize,
}

impl RunReport {
    pub fn is_convergent(&self) -> bool {
        match self.outcome {
            RunReportOutcome::Convergent { .. } => true,
            RunReportOutcome::Divergent { .. } => false,
        }
    }

    pub fn log(&self) -> &Self {
        tracing::info!("{:#?}", self);
        if self.is_convergent() {
            tracing::info!(
                "Reached equilibrium in {} mean iterations (variance {})",
                self.iteration_stats.mean,
                self.iteration_stats.variance
            );
        } else {
            tracing::warn!(
                "Divergent run found on attempt #{}. Failed to reach equilibrium in {} iterations",
                self.total_runs,
                DIVERGENCE_ITERS
            );
        }
        self
    }
}

#[derive(Clone, Debug)]
pub enum RunReportOutcome {
    /// The redundancy stats across just the last epoch of each run
    Convergent { redundancy_stats: Stats },
    /// The redundancy stats across the last N epochs of each run, all combined
    Divergent {
        redundancy_stats: Stats,
        num_epochs: usize,
    },
}

#[allow(dead_code)]
impl RunBatch {
    pub fn report(&self) -> RunReport {
        let num_epochs = 10;
        let iterations = DataVec::new(self.histories().map(|h| h.len() as f64).collect());
        let redundancies = DataVec::new(
            self.histories()
                .flatten()
                .map(|h| h.min_redundancy as f64)
                .collect(),
        );
        let outcome = match self.vergence() {
            Vergence::Convergent => RunReportOutcome::Convergent {
                redundancy_stats: Stats::new(DataVec::new(
                    self.histories()
                        .map(|hs| hs.last().unwrap().min_redundancy as f64)
                        .collect(),
                )),
            },
            Vergence::Divergent => RunReportOutcome::Divergent {
                num_epochs,
                redundancy_stats: Stats::new(DataVec::new(
                    self.histories()
                        .map(|hs| {
                            let mut hs = hs.clone();
                            hs.reverse();
                            hs.into_iter()
                                .take(num_epochs)
                                .map(|e| e.min_redundancy as f64)
                                .collect::<Vec<_>>()
                        })
                        .flatten()
                        .collect(),
                )),
            },
        };
        RunReport {
            iteration_stats: Stats::new(iterations),
            overall_redundancy_stats: Stats::new(redundancies),
            outcome,
            total_runs: self.histories().count(),
        }
    }

    pub fn vergence(&self) -> Vergence {
        if self.0.iter().all(|r| r.vergence.is_convergent()) {
            Vergence::Convergent
        } else {
            Vergence::Divergent
        }
    }

    pub fn runs(&self) -> &Vec<Run> {
        &self.0
    }

    pub fn histories(&self) -> impl Iterator<Item = &Vec<EpochStats>> + '_ {
        self.0.iter().map(|r| &r.history)
    }
}

#[derive(Debug)]
pub struct Run {
    pub vergence: Vergence,
    pub history: Vec<EpochStats>,
    /// the final state of the peers at the last iteration
    pub peers: Peers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vergence {
    Convergent,
    Divergent,
}

impl Vergence {
    pub fn is_convergent(&self) -> bool {
        *self == Vergence::Convergent
    }
}

impl PartialOrd for Vergence {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Vergence {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use Vergence::*;
        match (self, other) {
            (Divergent, Convergent) => std::cmp::Ordering::Less,
            (Convergent, Divergent) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EpochStats {
    pub net_delta_avg: f64,
    pub gross_delta_avg: f64,
    pub delta_max: f64,
    pub delta_min: f64,
    // pub delta_variance: f64,
    pub min_redundancy: u32,
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
    Ideal { target_coverage: f64 },
    Constant(f64),
    HalfAndHalf(f64, f64),
}

impl ArcLenStrategy {
    pub fn gen(&self, rng: &mut StdRng, num: usize) -> Vec<f64> {
        match self {
            Self::Random => iter::repeat_with(|| rng.gen()).take(num).collect(),
            Self::Ideal { target_coverage } => {
                iter::repeat((target_coverage / num as f64).min(1.0))
                    .take(num)
                    .collect()
            }
            Self::Constant(v) => iter::repeat(*v).take(num).collect(),
            Self::HalfAndHalf(a, b) => iter::repeat(*a)
                .take(num / 2)
                .chain(iter::repeat(*b).take(num / 2))
                .collect(),
        }
    }
}

/// View ascii for all arcs
pub fn print_arcs(arcs: &Peers) {
    for (i, arc) in arcs.into_iter().enumerate() {
        println!("|{}| {}", arc.to_ascii(64), i);
    }
}
