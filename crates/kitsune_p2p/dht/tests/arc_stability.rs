mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    arq::*,
    test_utils::{generate_ideal_coverage, generate_messy_coverage, seeded_rng},
};

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let detail = false;
    let n = 150;
    let j = 0.5;
    // let j = 10.0 / n as f64;
    let min_coverage = 50.0;

    let strat = ArqStrat {
        min_coverage,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_ideal_coverage(&mut rng, &strat, Some(10.0), n, j, 0);

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

#[test]
fn parameterized_stability_test_messy() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let detail = true;
    let n = 30;
    let j = 0.01;
    let len_mean = 0.25;
    let len_std = 0.20;
    // let j = 10.0 / n as f64;
    let min_coverage = 5.0;

    let strat = ArqStrat {
        min_coverage,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_messy_coverage(&mut rng, &strat, len_mean, len_std, n, j, 2);

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
    // assert!(pass_report(&report, rf * 2.0));
}
