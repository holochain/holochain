mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    arq::ArqStrat,
    test_utils::{generate_ideal_coverage, seeded_rng},
};

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 100;
    let j = 10.0 / n as f64;

    let r = 50;
    let rf = r as f64;

    let strat = ArqStrat::default();

    let peers = generate_ideal_coverage(&mut rng, &strat, None, n, j, 0);
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(2, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, 2);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    // assert!(pass_report(&report, rf * 2.0));
}
