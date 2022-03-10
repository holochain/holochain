mod common;

use common::quantized::*;
use kitsune_p2p_dht::{
    quantum::Topology,
    test_utils::{generate_ideal_coverage, generate_messy_coverage, seeded_rng},
    *,
};

fn pass_report(report: &RunReport, redundancy_target: f64) {
    match &report.outcome {
        RunReportOutcome::Convergent { redundancy_stats } => {
            pass_redundancy(redundancy_stats, redundancy_target)
        }
        _ => panic!("Divergent outcome is a failure"),
    }
}

/// Check if the min redundancy is "close enough" to the target for the given
/// Stats.
/// Currently this does not assert a very strong guarantee. Over time we want
/// to reduce the margins closer to zero.
fn pass_redundancy(stats: &Stats, redundancy_target: f64) {
    let rf = redundancy_target as f64;

    let margin_min = 0.40;
    let margin_median_lo = 0.40;

    assert!(
        stats.median >= rf * (1.0 - margin_median_lo),
        "median min redundancy too low: {}",
        stats.median
    );
    assert!(
        stats.min >= rf * (1.0 - margin_min),
        "minimum min redundancy too low: {}",
        stats.min
    );
}

/// Equilibrium test for a single distribution
#[test]
fn stability_test_case_near_ideal() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let topo = Topology::standard_zero();
    let detail = false;
    let mut rng = seeded_rng(None);
    let n = 150;
    let j = 0.1;
    let cov = 50.0;

    let strat = ArqStrat {
        min_coverage: cov,
        ..Default::default()
    };
    println!("{}", strat.summary());

    let peers = generate_ideal_coverage(&topo, &mut rng, &strat, Some(cov * 2.0), n, j);
    parameterized_stability_test(&topo, &strat, peers, detail);
}

#[test]
fn stability_test_case_messy() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let topo = Topology::standard_zero();
    let detail = true;

    let mut rng = seeded_rng(None);
    let n = 300;
    let j = 0.01;
    let len_mean = 0.50;
    let len_std = 0.35;
    let cov = 100.0;
    let strat = ArqStrat {
        min_coverage: cov,
        ..Default::default()
    };
    let peers = generate_messy_coverage(&topo, &mut rng, &strat, len_mean, len_std, n, j);
    parameterized_stability_test(&topo, &strat, peers, detail);
}

proptest::proptest! {

    #[test]
    #[ignore = "takes a very long time. run sparingly."]
    fn stability_test(num_peers in 100u32..300, min_coverage in 50.0f64..100.0, j in 0.0..1.0) {
        std::env::set_var("RUST_LOG", "debug");
        observability::test_run().ok();

        let topo = Topology::unit_zero();
        let detail = false;

        let mut rng = seeded_rng(None);

        let len_mean = 0.50;
        let len_std = 0.35;

        let strat = ArqStrat {
            min_coverage,
            ..Default::default()
        };

        let peers = generate_messy_coverage(&topo, &mut rng, &strat, len_mean, len_std, num_peers, j);
        parameterized_stability_test(&topo, &strat, peers, detail);
    }
}

fn parameterized_stability_test(topo: &Topology, strat: &ArqStrat, peers: Vec<Arq>, detail: bool) {
    println!("{}", strat.summary());

    if detail {
        println!("INITIAL CONDITIONS:");
        for (i, arq) in peers.iter().enumerate() {
            println!(
                "|{}| #{:<3} {:>3} {:>3}",
                arq.to_interval(topo).to_ascii(64),
                i,
                arq.count(),
                arq.power()
            );
        }
    }

    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(1, peers.clone(), |peers| {
        let (peers, stats) = run_one_epoch(topo, strat, peers, None, detail);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    pass_report(&report, strat.min_coverage);

    let actual_cov = actual_coverage(topo, eq.runs()[0].peers.iter());
    assert!(
        actual_cov >= strat.min_coverage,
        "{} < {}",
        actual_cov,
        strat.min_coverage
    );
    assert!(
        actual_cov <= strat.max_coverage() + 1.0,
        "{} > {}",
        actual_cov,
        strat.max_coverage() + 1.0
    );
}
