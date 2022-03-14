#![cfg(feature = "testing")]
#![cfg(feature = "NORUN")]

mod common;

use std::collections::HashSet;

use common::continuous::*;
use kitsune_p2p_dht::arq::PeerStrat;
use kitsune_p2p_dht_arc::*;

fn pass_report(report: &RunReport, redundancy_target: f64) -> bool {
    match &report.outcome {
        RunReportOutcome::Convergent { redundancy_stats } => {
            pass_redundancy(redundancy_stats, redundancy_target)
        }
        RunReportOutcome::Divergent {
            redundancy_stats, ..
        } => pass_redundancy(redundancy_stats, redundancy_target),
    }
}

/// Check if the min redundancy is "close enough" to the target for the given
/// Stats.
/// Currently this does not assert a very strong guarantee. Over time we want
/// to reduce the margins closer to zero.
fn pass_redundancy(stats: &Stats, redundancy_target: f64) -> bool {
    let rf = redundancy_target as f64;

    let margin_min = 0.40;
    let margin_median_lo = 0.40;
    let margin_median_hi = 0.20;
    stats.median >= rf * (1.0 - margin_median_lo)
        && stats.median <= rf * (1.0 + margin_median_hi)
        && stats.min >= rf * (1.0 - margin_min)
}

#[test]
#[ignore = "Not suitable for ci"]
fn single_agent_convergence_debug() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let n = 1000;
    let redundancy = 100;
    let j = 0.1;
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
    peers[0] = DhtArc::Full(peers[0].start_loc());
    tracing::debug!("{}", EpochStats::oneline_header());
    let runs = determine_equilibrium(1, peers, |peers| {
        let dynamic = Some(maplit::hashset![0]);
        let (peers, stats) = run_one_epoch(&strat, peers, dynamic.as_ref(), DETAIL);

        tracing::debug!("{}", stats.oneline());
        // tracing::debug!("{}", peers[0].coverage());
        (peers, stats)
    });
    // print_arcs(&runs.runs()[0].peers);
    let report = runs.report();
    report.log();
    assert!(report.is_convergent());
}

pub fn run_report(
    strat: &PeerStrat,
    indices: &Option<HashSet<usize>>,
    iters: usize,
    n: usize,
    j: f64,
    s: ArcLenStrategy,
) -> RunReport {
    tracing::info!("");
    tracing::info!("------------------------");

    // let seed = None;
    let seed = Some(7532095396949412554);
    let mut rng = seeded_rng(seed);

    let mut peers = simple_parameterized_generator(&mut rng, n, j, s);
    peers[0] = DhtArc::Full(peers[0].start_loc());
    let runs = determine_equilibrium(iters, peers, |peers| {
        let (peers, stats) = run_one_epoch(strat, peers, indices.as_ref(), DETAIL);
        tracing::debug!("{}", peers[0].coverage());
        (peers, stats)
    });
    let report = runs.report();
    if DETAIL >= 2 {
        print_arcs(&runs.runs()[0].peers);
    }
    if DETAIL >= 1 {
        report.log();
    }
    report
}

/// Test if various distributions of agents can converge
#[test]
#[cfg(feature = "slow_tests")]
fn parameterized_battery() {
    std::env::set_var("RUST_LOG", "info");
    observability::test_run().ok();
    use std::collections::HashSet;

    let n = 100;
    let r = 50;
    let rf = r as f64;
    let s = ArcLenStrategy::Constant(1.0);
    let its = 3;

    let pass = |report: RunReport| pass_report(&report, rf);
    let pass_convergent = |report: RunReport| report.is_convergent() && pass_report(&report, rf);
    let _pass_divergent = |report: RunReport| !report.is_convergent() && pass_report(&report, rf);

    let strat_alpha_0: PeerStrat = PeerStratAlpha {
        check_gaps: false,
        redundancy_target: r / 2,
        ..Default::default()
    }
    .into();

    let strat_alpha_1: PeerStrat = PeerStratAlpha {
        check_gaps: true,
        redundancy_target: r / 2,
        ..Default::default()
    }
    .into();

    let strat_beta: PeerStrat = PeerStratBeta {
        min_sample_size: r / 2,
        ..Default::default()
    }
    .into();

    // If None, resize all arcs. If Some, only resize the specified indices.
    let ixs: Option<HashSet<usize>> = None;

    // beta
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.0, s)));
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.01, s)));
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.05, s)));
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.1, s)));
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.25, s)));
    assert!(pass(run_report(&strat_beta, &ixs, its, n, 0.5, s)));

    // alpha, gap_check == true
    pass_convergent(run_report(&strat_alpha_1, &ixs, its, n, 0.0, s));
    pass_convergent(run_report(&strat_alpha_1, &ixs, its, n, 0.001, s));
    pass_convergent(run_report(&strat_alpha_1, &ixs, its, n, 0.01, s));

    // alpha, gap_check == false
    pass_convergent(run_report(&strat_alpha_0, &ixs, its, n, 0.0, s));
    pass_convergent(run_report(&strat_alpha_0, &ixs, its, n, 0.001, s));
    pass_convergent(run_report(&strat_alpha_0, &ixs, its, n, 0.01, s));
    // Note that the following cases fail with gap_check
    pass(run_report(&strat_alpha_0, &ixs, its, n, 0.1, s));
    pass(run_report(&strat_alpha_0, &ixs, its, n, 0.5, s));
    pass(run_report(&strat_alpha_0, &ixs, its, n, 1.0, s));
}

/// Equilibrium test for a single distribution
#[test]
#[ignore = "Not suitable for ci"]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 1000;
    let j = 10.0 / n as f64;
    let s = ArcLenStrategy::Constant(0.1);

    let r = 50;
    let rf = r as f64;

    let strat = PeerStratAlpha {
        redundancy_target: r,
        ..Default::default()
    }
    .into();

    let peers = simple_parameterized_generator(&mut rng, n, j, s);
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    print_arcs(&peers);
    let eq = determine_equilibrium(2, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, DETAIL);
        tracing::debug!("{}", stats.oneline());
        print_arcs(&peers);
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    assert!(pass_report(&report, rf * 2.0));
}

/// Equilibrium test for a single distribution
#[test]
#[ignore = "Not suitable for ci"]
fn test_peer_view_beta() {
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 1000;
    let j = 10.0 / n as f64;
    let s = ArcLenStrategy::Constant(0.1);

    let r = 50;
    let strat = PeerStratBeta {
        min_sample_size: r,
        ..Default::default()
    };

    let target_redundancy = r as f64 / strat.default_uptime;
    let error_buffer = target_redundancy as f64 * strat.total_coverage_buffer;
    let min_r = target_redundancy - error_buffer;

    let peers = simple_parameterized_generator(&mut rng, n, j, s);
    tracing::debug!("{}", EpochStats::oneline_header());
    let runs = determine_equilibrium(2, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat.into(), peers, None, DETAIL);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    dbg!(min_r);
    assert!(pass_report(runs.report().log(), min_r));
}
