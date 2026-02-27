//! Multi-cell startup contention test.
//!
//! Simulates a conductor with 13 cells (each on a different DNA) restarting,
//! then immediately being bombarded with concurrent zome calls and
//! dump_network_metrics requests across all cells simultaneously.
//! This is closer to field conditions where many hApps are installed.

#![cfg(feature = "test_utils")]

use super::{create_link_zomes, MetricsPollResult, PollResult};
use holochain::sweettest::*;
use holochain_types::network::Kitsune2NetworkMetricsRequest;
use holochain_types::prelude::*;
use holochain_zome_types::link::Link;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const NUM_CELLS: usize = 13;

/// Per-cell results collected during polling.
struct CellPollResults {
    cell_index: usize,
    local_results: Vec<PollResult>,
    network_results: Vec<PollResult>,
}

/// Poll a single cell's get_links (both local and network) concurrently.
async fn poll_cell(
    conductor: &SweetConductor,
    zome: &SweetZome,
    base: ActionHash,
    cell_index: usize,
    restart_instant: Instant,
    timeout: Duration,
    interval: Duration,
) -> CellPollResults {
    let (local_results, network_results) = tokio::join!(
        super::poll_get_links(
            conductor,
            zome,
            "get_links_local",
            "local",
            base.clone(),
            restart_instant,
            timeout,
            interval,
        ),
        super::poll_get_links(
            conductor,
            zome,
            "get_links_network",
            "network",
            base,
            restart_instant,
            timeout,
            interval,
        ),
    );
    CellPollResults {
        cell_index,
        local_results,
        network_results,
    }
}

/// Poll dump_network_metrics for ALL DNAs simultaneously.
async fn poll_all_metrics(
    conductor: &SweetConductor,
    dna_hashes: &[DnaHash],
    restart_instant: Instant,
    timeout: Duration,
    interval: Duration,
    stop: Arc<AtomicBool>,
) -> Vec<MetricsPollResult> {
    let mut results = Vec::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        let elapsed = restart_instant.elapsed();

        // Call dump_network_metrics with no DNA filter — gets all DNAs at once.
        let metrics_result = conductor
            .raw_handle()
            .dump_network_metrics(Kitsune2NetworkMetricsRequest {
                dna_hash: None,
                include_dht_summary: false,
            })
            .await;

        let (peer_count, error) = match &metrics_result {
            Ok(metrics) => {
                // Sum up peers across all DNAs.
                let total: usize = dna_hashes
                    .iter()
                    .filter_map(|dh| metrics.get(dh))
                    .map(|m| m.gossip_state_summary.peer_meta.len())
                    .sum();
                (Some(total), None)
            }
            Err(e) => (None, Some(format!("{:?}", e))),
        };

        results.push(MetricsPollResult {
            elapsed_since_restart: elapsed,
            peer_count,
            error,
        });

        tokio::time::sleep(interval).await;
    }

    results
}

fn print_multi_cell_report(
    test_name: &str,
    cell_results: &[CellPollResults],
    metrics_results: &[MetricsPollResult],
) {
    println!("\n========================================");
    println!("  REPORT: {}", test_name);
    println!("  ({} cells)", cell_results.len());
    println!("========================================");

    // Per-cell summary table.
    println!("\n  --- Per-Cell Summary ---");
    println!(
        "    {:>5}  {:>12}  {:>12}  {:>8}  {:>8}",
        "Cell", "Local 1st(ms)", "Net 1st(ms)", "L Errs", "N Errs"
    );

    let mut any_local_timeout = false;
    let mut any_network_timeout = false;
    let mut max_local_ms: u128 = 0;
    let mut max_network_ms: u128 = 0;

    for cr in cell_results {
        let local_first = cr
            .local_results
            .iter()
            .find(|r| r.link_count.map_or(false, |c| c > 0));
        let network_first = cr
            .network_results
            .iter()
            .find(|r| r.link_count.map_or(false, |c| c > 0));
        let local_errors = cr.local_results.iter().filter(|r| r.error.is_some()).count();
        let network_errors = cr
            .network_results
            .iter()
            .filter(|r| r.error.is_some())
            .count();

        let local_ms = match local_first {
            Some(r) => {
                let ms = r.elapsed_since_restart.as_millis();
                if ms > max_local_ms {
                    max_local_ms = ms;
                }
                format!("{}", ms)
            }
            None => {
                any_local_timeout = true;
                "TIMEOUT".to_string()
            }
        };
        let network_ms = match network_first {
            Some(r) => {
                let ms = r.elapsed_since_restart.as_millis();
                if ms > max_network_ms {
                    max_network_ms = ms;
                }
                format!("{}", ms)
            }
            None => {
                any_network_timeout = true;
                "TIMEOUT".to_string()
            }
        };

        println!(
            "    {:>5}  {:>12}  {:>12}  {:>8}  {:>8}",
            cr.cell_index, local_ms, network_ms, local_errors, network_errors
        );
    }

    // Show any cells that had errors (first error only per cell for brevity).
    let cells_with_errors: Vec<_> = cell_results
        .iter()
        .filter(|cr| {
            cr.local_results.iter().any(|r| r.error.is_some())
                || cr.network_results.iter().any(|r| r.error.is_some())
        })
        .collect();

    if !cells_with_errors.is_empty() {
        println!("\n  --- Sample Errors ---");
        for cr in cells_with_errors.iter().take(3) {
            if let Some(first_err) = cr
                .local_results
                .iter()
                .chain(cr.network_results.iter())
                .find_map(|r| r.error.as_ref())
            {
                println!("    Cell {}: {}", cr.cell_index, first_err);
            }
        }
        if cells_with_errors.len() > 3 {
            println!("    ... and {} more cells with errors", cells_with_errors.len() - 3);
        }
    }

    // Metrics summary.
    super::print_metrics_results(metrics_results);

    // Overall summary.
    println!("\n  --- Overall Summary ---");
    println!("    Total cells: {}", cell_results.len());
    if any_local_timeout {
        println!("    Local:   SOME CELLS TIMED OUT");
    } else {
        println!("    Local:   all cells got data, worst case {}ms", max_local_ms);
    }
    if any_network_timeout {
        println!("    Network: SOME CELLS TIMED OUT");
    } else {
        println!(
            "    Network: all cells got data, worst case {}ms",
            max_network_ms
        );
    }
    println!("========================================\n");
}

/// Test: Restart a conductor with 13 cells across 13 different DNAs.
/// Immediately bombard with concurrent zome calls to ALL cells + metrics dumps.
///
/// This simulates field conditions where many hApps are installed on one conductor.
/// With 13 cells, the conductor has to:
/// - Create cells in batches of 5 (3 waves)
/// - Join the network for each cell in batches of 10 (2 waves)
/// - Run init callbacks for each cell
/// - Handle gossip for each DNA
/// All while servicing incoming zome calls and metrics requests.
#[tokio::test(flavor = "multi_thread")]
async fn test_restart_13_cells_contention() {
    holochain_trace::test_run();

    // Phase 1: Create 13 unique DNAs (each with the same zome code).
    let mut dna_files = Vec::with_capacity(NUM_CELLS);
    for _ in 0..NUM_CELLS {
        let zomes = create_link_zomes();
        let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
        dna_files.push(dna_file);
    }

    println!("Created {} unique DNAs.", NUM_CELLS);

    // Phase 2: Set up two conductors, install all 13 DNAs on each.
    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        2,
        SweetConductorConfig::rendezvous(true),
    )
    .await;

    // Install each DNA as a separate app on both conductors.
    let mut alice_cells = Vec::with_capacity(NUM_CELLS);
    let mut bob_cells = Vec::with_capacity(NUM_CELLS);
    let mut dna_hashes = Vec::with_capacity(NUM_CELLS);

    for (i, dna_file) in dna_files.iter().enumerate() {
        let app_id = format!("app_{}", i);
        let apps = conductors
            .setup_app(&app_id, [dna_file])
            .await
            .unwrap();
        let ((alice_cell,), (bob_cell,)) = apps.into_tuples();
        dna_hashes.push(alice_cell.dna_hash().clone());
        alice_cells.push(alice_cell);
        bob_cells.push(bob_cell);
    }

    println!("Installed {} apps on 2 conductors ({} cells each).", NUM_CELLS, NUM_CELLS);

    // Phase 3: Create entry + link on Alice for each DNA, then sync.
    let mut base_hashes = Vec::with_capacity(NUM_CELLS);
    for (i, alice_cell) in alice_cells.iter().enumerate() {
        let alice_zome = alice_cell.zome(SweetInlineZomes::COORDINATOR);
        let base_hash: ActionHash = conductors[0].call(&alice_zome, "create_entry", ()).await;
        let _link_hash: ActionHash = conductors[0]
            .call(&alice_zome, "create_link", base_hash.clone())
            .await;
        base_hashes.push(base_hash);

        // Declare full storage arcs for both conductors on this DNA.
        conductors[0]
            .declare_full_storage_arcs(alice_cell.dna_hash())
            .await;
        conductors[1]
            .declare_full_storage_arcs(bob_cells[i].dna_hash())
            .await;
    }
    conductors.exchange_peer_info().await;

    println!("Created data on all {} cells. Waiting for consistency...", NUM_CELLS);

    // Wait for consistency on each DNA pair.
    for i in 0..NUM_CELLS {
        await_consistency([&alice_cells[i], &bob_cells[i]])
            .await
            .unwrap();
    }

    // Verify baseline: Bob can see the link on every cell.
    for (i, bob_cell) in bob_cells.iter().enumerate() {
        let bob_zome = bob_cell.zome(SweetInlineZomes::COORDINATOR);
        let baseline: Vec<Link> = conductors[1]
            .call(&bob_zome, "get_links_local", base_hashes[i].clone())
            .await;
        assert_eq!(
            baseline.len(),
            1,
            "Baseline: Bob cell {} should see 1 link",
            i
        );
    }

    println!("Baseline verified for all {} cells. Shutting down Bob...", NUM_CELLS);

    // Phase 4: Shutdown and restart Bob.
    conductors[1].shutdown().await;
    println!("Bob shut down. Restarting...");
    conductors[1].startup(false).await;
    let restart_instant = Instant::now();
    println!(
        "Bob restarted with {} cells. Starting concurrent polling on ALL cells...",
        NUM_CELLS
    );

    // Phase 5: Bombard with concurrent zome calls across ALL cells + metrics dumps.
    let poll_timeout = Duration::from_secs(120);
    let poll_interval = Duration::from_millis(200);

    let metrics_stop = Arc::new(AtomicBool::new(false));
    let metrics_stop_clone = metrics_stop.clone();

    // Build futures for all cells.
    let bob_zomes: Vec<_> = bob_cells
        .iter()
        .map(|c| c.zome(SweetInlineZomes::COORDINATOR))
        .collect();

    let cell_futures: Vec<_> = (0..NUM_CELLS)
        .map(|i| {
            poll_cell(
                &conductors[1],
                &bob_zomes[i],
                base_hashes[i].clone(),
                i,
                restart_instant,
                poll_timeout,
                poll_interval,
            )
        })
        .collect();

    // Run all cell polling + metrics polling concurrently.
    let (cell_results, metrics_results) = tokio::join!(
        async {
            let results = futures::future::join_all(cell_futures).await;
            metrics_stop_clone.store(true, Ordering::Relaxed);
            results
        },
        poll_all_metrics(
            &conductors[1],
            &dna_hashes,
            restart_instant,
            poll_timeout,
            Duration::from_millis(500),
            metrics_stop,
        ),
    );

    // Phase 6: Report.
    print_multi_cell_report(
        "13-Cell Restart Contention",
        &cell_results,
        &metrics_results,
    );

    // Assertions: every cell should eventually return data for local strategy.
    for cr in &cell_results {
        assert!(
            cr.local_results.iter().any(|r| r.link_count == Some(1)),
            "Cell {} local strategy should return 1 link after restart",
            cr.cell_index,
        );
    }

    // Network strategy: report but don't assert (may legitimately timeout under contention).
    let network_timeouts: Vec<_> = cell_results
        .iter()
        .filter(|cr| !cr.network_results.iter().any(|r| r.link_count == Some(1)))
        .map(|cr| cr.cell_index)
        .collect();
    if !network_timeouts.is_empty() {
        println!(
            "WARNING: {} cells timed out on network strategy: {:?}",
            network_timeouts.len(),
            network_timeouts
        );
    }
}
