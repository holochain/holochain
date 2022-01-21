mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    arq::*,
    test_utils::{generate_ideal_coverage, seeded_rng},
};

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let detail = false;
    let n = 150;
    let j = 1.0;
    // let j = 10.0 / n as f64;
    let min_coverage = 50.0;

    let strat = ArqStrat {
        min_coverage,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_ideal_coverage(&mut rng, &strat, None, n, j, 0);
    let view = PeerView::new(strat.clone(), ArqSet::new(peers.clone()));
    let cov = view.extrapolated_coverage(&Arq::new_full(0.into(), 27).to_bounds());
    assert!(strat.min_coverage <= cov);
    assert!(cov <= strat.max_coverage());
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(1, peers.clone(), |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, detail);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    // assert!(pass_report(&report, rf * 2.0));
}
