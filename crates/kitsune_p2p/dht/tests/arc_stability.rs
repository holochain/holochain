mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    arq::{print_arqs, Arq, ArqSet, ArqStrat, PeerView},
    test_utils::{generate_ideal_coverage, seeded_rng},
};

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 100;
    // let j = 0.0;
    let j = 10.0 / n as f64;

    let r = 50;
    let rf = r as f64;

    let strat = ArqStrat::default();
    println!("{}", strat.summary());

    let peers = generate_ideal_coverage(&mut rng, &strat, None, n, j, 0);
    let view = PeerView::new(strat.clone(), ArqSet::new(peers.clone()));
    let cov = view.extrapolated_coverage(&Arq::new_full(0.into(), 27).to_bounds());
    assert!(strat.min_coverage <= cov);
    assert!(cov <= strat.max_coverage());
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(1, peers.clone(), |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, 1);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    // print_arqs(&eq.runs()[0].peers, 64);
    let report = eq.report();
    report.log();
    // assert!(pass_report(&report, rf * 2.0));
}
