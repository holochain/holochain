mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    arq::*,
    test_utils::{generate_ideal_coverage, generate_messy_coverage, seeded_rng},
};

fn pass_report(report: &RunReport, redundancy_target: f64) -> bool {
    pass_redundancy(&report.overall_redundancy_stats, redundancy_target);
    match &report.outcome {
        RunReportOutcome::Convergent { redundancy_stats } => {
            pass_redundancy(redundancy_stats, redundancy_target)
        }
        _ => false
        // RunReportOutcome::Divergent {
        //     redundancy_stats, ..
        // } => pass_redundancy(redundancy_stats, redundancy_target),
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

/// Equilibrium test for a single distribution
#[test]
fn stability_test_near_ideal() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let detail = true;
    let n = 150;
    let j = 0.1;
    // let j = 10.0 / n as f64;
    let min_coverage = 50.0;

    let strat = ArqStrat {
        min_coverage,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_ideal_coverage(&mut rng, &strat, Some(100.0), n, j, 0);

    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(1, peers.clone(), |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, detail);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    assert!(pass_report(&report, min_coverage));
}

#[test]
fn stability_test_messy() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let detail = true;
    let n = 300;
    let j = 0.01;
    let len_mean = 0.50;
    let len_std = 0.35;
    let min_coverage = 100.0;

    let strat = ArqStrat {
        min_coverage,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_messy_coverage(&mut rng, &strat, len_mean, len_std, n, j, 0);

    println!("INITIAL CONDITIONS:");
    for (i, arq) in peers.iter().enumerate() {
        println!(
            "|{}| #{:<3} {:>3} {:>3}",
            arq.to_interval().to_ascii(64),
            i,
            arq.count(),
            arq.power()
        );
    }
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(1, peers.clone(), |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, detail);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    assert!(pass_report(&report, min_coverage));
}
