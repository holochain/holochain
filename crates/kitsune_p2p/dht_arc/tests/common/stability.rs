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
const DIVERGENCE_ITERS: usize = 30;

/// Number of consecutive rounds of no movement before declaring convergence.
const CONVERGENCE_WINDOW: usize = 3;

/// Level of detail in reporting.
pub const DETAIL: u8 = 0;

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

#[allow(dead_code)]
pub fn run_single_agent_convergence(
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
    *peers[0].half_length_mut() = MAX_HALF_LENGTH;
    let runs = determine_equilibrium(iters, peers, |peers| {
        let dynamic = Some(maplit::hashset![0]);
        let (peers, stats) = run_one_epoch(&strat, peers, dynamic.as_ref(), DETAIL);
        tracing::debug!("{}", peers[0].coverage());
        (peers, stats)
    });
    report(&runs);
    runs
}

pub fn report(e: &RunBatch) {
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

pub fn determine_oscillations<'a, F, T>(
    iters: usize,
    peers: Peers,
    step: F,
    mut targets: T,
) -> OscillationsBatch
where
    F: 'a + Clone + Fn(Peers) -> (Peers, EpochStats),
    T: 'a + Clone + FnMut(EpochStats) -> bool,
{
    let mut runs = vec![];
    for i in 1..=iters {
        tracing::debug!("----- Running movement iteration {} -----", i);
        let run = record_oscillations(peers.clone(), |peers| step(peers), &mut targets);
        runs.push(run);
    }
    OscillationsBatch(runs)
}

/// Run iterations until there is no movement of any arc
pub fn record_oscillations<'a, F, T>(mut peers: Peers, step: F, targets: &mut T) -> Oscillations
where
    F: Fn(Peers) -> (Peers, EpochStats),
    T: FnMut(EpochStats) -> bool,
{
    let mut history = Vec::with_capacity(DIVERGENCE_ITERS);
    let mut num_missed = 0;
    for _ in 0..DIVERGENCE_ITERS {
        let (p, stats) = step(peers);
        peers = p;
        history.push(stats.clone());
        if targets(stats) {
            num_missed += 1;
            if num_missed > CONVERGENCE_WINDOW {
                return Oscillations { history, peers };
            }
        }
    }

    Oscillations { history, peers }
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
pub struct RunBatch(Vec<Run>);

#[derive(Debug)]
pub struct OscillationsBatch(pub Vec<Oscillations>);

#[allow(dead_code)]
impl RunBatch {
    pub fn vergence(&self) -> Vergence {
        if self.0.iter().all(|r| r.vergence == Vergence::Convergent) {
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
pub struct Run {
    pub vergence: Vergence,
    pub history: Vec<EpochStats>,
    /// the final state of the peers at the last iteration
    pub peers: Peers,
}

#[derive(Debug)]
pub struct Oscillations {
    pub history: Vec<EpochStats>,
    /// the final state of the peers at the last iteration
    pub peers: Peers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vergence {
    Convergent,
    Divergent,
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
pub fn print_arcs(arcs: &Peers) {
    for (i, arc) in arcs.into_iter().enumerate() {
        println!("|{}| {}", arc.to_ascii(64), i);
    }
}

/// Wait for input, to slow down overwhelmingly large iterations
pub fn get_input() {
    let mut input_string = String::new();
    std::io::stdin()
        .read_line(&mut input_string)
        .ok()
        .expect("Failed to read line");
}
