mod common;

use common::stability::*;
use kitsune_p2p_dht_arc::*;

use pretty_assertions::assert_eq;

fn pass_report(report: &RunReport, redundancy_target: u16) -> bool {
    match &report.outcome {
        RunReportOutcome::Convergent { redundancy_stats } => {
            pass_redundancy(redundancy_stats, redundancy_target)
        }
        RunReportOutcome::Divergent => pass_redundancy(&report.redundancy_stats, redundancy_target),
    }
}

/// Check if the redundancy is close to the target for the entire duration
/// of all runs.
/// - Min redundancy must never dip below 95% of the target
/// - Median redundancy must be within 1% of target
fn pass_redundancy(stats: &Stats, redundancy_target: u16) -> bool {
    let rf = redundancy_target as f64;
    stats.median >= rf * 0.99
        && stats.median <= rf * 1.01
        && stats.min >= rf * 0.9
        && stats.variance < 1.0
}

#[test]
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
    *peers[0].half_length_mut() = MAX_HALF_LENGTH;
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

/// Test if various distributions of agents can converge
#[test]
#[cfg(feature = "slow_tests")]
fn single_agent_convergence_battery() {
    std::env::set_var("RUST_LOG", "info");
    observability::test_run().ok();
    use Vergence::*;

    let n = 1000;
    let r = 100;

    let pass_convergent =
        |report: RunReport| report.log().is_convergent() && pass_report(&report, r);
    let pass_divergent =
        |report: RunReport| !report.log().is_convergent() && pass_report(&report, r);

    // let divergent = vec![
    //     pass_divergent(run_single_agent_convergence(8, n, r, 0.1, true).report()),
    //     pass_divergent(run_single_agent_convergence(8, n, r, 0.5, true).report()),
    //     pass_divergent(run_single_agent_convergence(8, n, r, 1.0, true).report()),
    // ];
    // assert_eq!(divergent, vec![true; divergent.len()]);

    let convergent = vec![
        // gap_check == true
        pass_convergent(run_single_agent_convergence(8, n, r, 0.0, true).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 0.001, true).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 0.01, true).report()),
        // gap_check == false
        pass_convergent(run_single_agent_convergence(8, n, r, 0.0, false).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 0.001, false).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 0.01, false).report()),
        // Note that these same scenarios fail to converge with gap_check
        pass_convergent(run_single_agent_convergence(8, n, r, 0.1, false).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 0.5, false).report()),
        pass_convergent(run_single_agent_convergence(8, n, r, 1.0, false).report()),
    ];

    assert_eq!(convergent, vec![true; convergent.len()]);
}

/// Equilibrium test for a single distribution
#[test]
fn parameterized_stability_test() {
    std::env::set_var("RUST_LOG", "debug");
    observability::test_run().ok();

    let mut rng = seeded_rng(None);

    let n = 1000;
    let j = 10.0 / n as f64;
    let s = ArcLenStrategy::Constant(0.1);

    let r = 50;
    let strat = PeerStratAlpha {
        redundancy_target: r,
        ..Default::default()
    }
    .into();

    let peers = simple_parameterized_generator(&mut rng, n, j, s);
    tracing::info!("");
    tracing::debug!("{}", EpochStats::oneline_header());
    let eq = determine_equilibrium(2, peers, |peers| {
        let (peers, stats) = run_one_epoch(&strat, peers, None, DETAIL);
        tracing::debug!("{}", stats.oneline());
        (peers, stats)
    });
    let report = eq.report();
    report.log();
    assert!(pass_report(&report, r * 2));
}

/// Equilibrium test for a single distribution
#[test]
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
    let report = determine_oscillations(
        2,
        peers,
        |peers| {
            let (peers, stats) = run_one_epoch(&strat.into(), peers, None, DETAIL);
            tracing::debug!("{}", stats.oneline());
            (peers, stats)
        },
        |EpochStats {
             net_delta_avg: _,
             gross_delta_avg: _,
             delta_max: _,
             delta_min: _,
             min_redundancy,
         }| { (min_redundancy as f64) < min_r },
    );
    for run in report.0 {
        for peer in &run.peers {
            let view = strat.view(*peer, run.peers.as_slice());

            dbg!(view.count);
            if view.target_coverage() > 0.9 {
                dbg!(view.est_total_coverage());
                dbg!(view.strat.target_network_coverage() - view.est_total_coverage());
                dbg!(view.count);
            }
            // dbg!(view.est_total_coverage());
        }
        for &EpochStats {
            net_delta_avg: _,
            gross_delta_avg: _,
            delta_max: _,
            delta_min: _,
            min_redundancy,
        } in &run.history
        {
            assert!(
                min_redundancy as f64 >= min_r,
                "{} >= {}",
                min_redundancy,
                min_r
            );
        }
    }
    // dbg!(report);
    // report(&eq);
    // eq.assert_convergent();
    // // TODO: the min redundancy is never exactly 100.
    // //       would be good to look at the *average* redundancy, and other stats.
    // eq.assert_min_redundancy(96);
}
