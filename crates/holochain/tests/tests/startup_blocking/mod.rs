//! Integration tests for zome call behavior during conductor restart.
//!
//! Tests that after a conductor restarts with previously installed cells,
//! zome calls work correctly even before networking is fully re-established.
//! Compares behavior between GetStrategy::Local (local DHT only) and
//! GetStrategy::Network (fetch from peers), and also polls network metrics
//! to observe networking state during the race window.

#![cfg(feature = "test_utils")]

mod multi_cell;
mod websocket;

use holo_hash::fixt::ActionHashFixturator;
use holochain::sweettest::*;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::network::Kitsune2NetworkMetricsRequest;
use holochain_types::prelude::*;
use holochain_zome_types::entry::GetOptions;
use holochain_zome_types::link::{GetLinksInput, Link};
use holochain_zome_types::prelude::LinkQuery;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Create inline zomes with create_entry, create_link, and two get_links variants
/// (local-only and network).
fn create_link_zomes() -> SweetInlineZomes {
    let entry_def = EntryDef::default_from_id("test_entry");
    SweetInlineZomes::new(vec![entry_def], 1)
        .function("create_entry", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("create_link", move |api, base: ActionHash| {
            let target = ::fixt::fixt!(ActionHash);
            let hash = api.create_link(CreateLinkInput::new(
                base.into(),
                target.into(),
                0.into(),
                0.into(),
                vec![].into(),
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get_links_local", move |api, base: ActionHash| {
            let query = LinkQuery::new(base, LinkTypeFilter::single_type(0.into(), 0.into()));
            let links =
                api.get_links(vec![GetLinksInput::from_query(query, GetOptions::local())])?;
            Ok(links.into_iter().flatten().collect::<Vec<_>>())
        })
        .function("get_links_network", move |api, base: ActionHash| {
            let query = LinkQuery::new(base, LinkTypeFilter::single_type(0.into(), 0.into()));
            let links =
                api.get_links(vec![GetLinksInput::from_query(query, GetOptions::network())])?;
            Ok(links.into_iter().flatten().collect::<Vec<_>>())
        })
}

#[derive(Debug)]
struct PollResult {
    elapsed_since_restart: Duration,
    strategy: &'static str,
    link_count: Option<usize>,
    error: Option<String>,
}

#[derive(Debug)]
struct MetricsPollResult {
    elapsed_since_restart: Duration,
    peer_count: Option<usize>,
    error: Option<String>,
}

/// Poll get_links repeatedly until we get data or timeout.
/// Returns all results collected during polling.
async fn poll_get_links(
    conductor: &SweetConductor,
    zome: &SweetZome,
    fn_name: &str,
    strategy_label: &'static str,
    base: ActionHash,
    restart_instant: Instant,
    timeout: Duration,
    interval: Duration,
) -> Vec<PollResult> {
    let mut results = Vec::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let elapsed = restart_instant.elapsed();
        let result: Result<Vec<Link>, _> =
            conductor.call_fallible(&zome, fn_name, base.clone()).await;

        let (link_count, error) = match &result {
            Ok(links) => (Some(links.len()), None),
            Err(e) => (None, Some(format!("{:?}", e))),
        };

        let has_data = link_count.map_or(false, |c| c > 0);

        results.push(PollResult {
            elapsed_since_restart: elapsed,
            strategy: strategy_label,
            link_count,
            error,
        });

        if has_data {
            // Got data. Fire a few more confirmation calls, then stop.
            for _ in 0..3 {
                tokio::time::sleep(interval).await;
                let elapsed = restart_instant.elapsed();
                let result: Result<Vec<Link>, _> =
                    conductor.call_fallible(&zome, fn_name, base.clone()).await;
                let (link_count, error) = match &result {
                    Ok(links) => (Some(links.len()), None),
                    Err(e) => (None, Some(format!("{:?}", e))),
                };
                results.push(PollResult {
                    elapsed_since_restart: elapsed,
                    strategy: strategy_label,
                    link_count,
                    error,
                });
            }
            break;
        }

        tokio::time::sleep(interval).await;
    }

    results
}

/// Poll dump_network_metrics repeatedly during the test window.
/// Stops when the stop flag is set or the timeout is reached.
async fn poll_network_metrics(
    conductor: &SweetConductor,
    dna_hash: &DnaHash,
    restart_instant: Instant,
    timeout: Duration,
    interval: Duration,
    stop: Arc<AtomicBool>,
) -> Vec<MetricsPollResult> {
    let mut results = Vec::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        let elapsed = restart_instant.elapsed();
        let metrics_result = conductor
            .raw_handle()
            .dump_network_metrics(Kitsune2NetworkMetricsRequest {
                dna_hash: Some(dna_hash.clone()),
                include_dht_summary: false,
            })
            .await;

        let (peer_count, error) = match &metrics_result {
            Ok(metrics) => {
                let count = metrics
                    .get(dna_hash)
                    .map(|m| m.gossip_state_summary.peer_meta.len())
                    .unwrap_or(0);
                (Some(count), None)
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

fn print_results(label: &str, results: &[PollResult]) {
    println!("\n  --- {} ({} calls) ---", label, results.len());
    for r in results {
        let elapsed_ms = r.elapsed_since_restart.as_millis();
        match (&r.link_count, &r.error) {
            (Some(count), _) => {
                let marker = if *count > 0 { "OK" } else { "EMPTY" };
                println!(
                    "    [{:>6}ms] {:>7} => {:>5} ({} links)",
                    elapsed_ms, r.strategy, marker, count
                );
            }
            (_, Some(err)) => {
                println!(
                    "    [{:>6}ms] {:>7} => ERROR ({})",
                    elapsed_ms, r.strategy, err
                );
            }
            _ => {}
        }
    }

    let first_with_data = results
        .iter()
        .find(|r| r.link_count.map_or(false, |c| c > 0));
    let empty_count = results
        .iter()
        .filter(|r| r.link_count == Some(0))
        .count();
    let error_count = results.iter().filter(|r| r.error.is_some()).count();

    if let Some(first) = first_with_data {
        println!(
            "  => {}: first data at {}ms ({} empty, {} errors before)",
            label,
            first.elapsed_since_restart.as_millis(),
            empty_count,
            error_count,
        );
    } else {
        println!(
            "  => {}: NO data returned after {} calls ({} empty, {} errors)",
            label,
            results.len(),
            empty_count,
            error_count,
        );
    }
}

fn print_metrics_results(results: &[MetricsPollResult]) {
    println!(
        "\n  --- Network Metrics ({} samples) ---",
        results.len()
    );
    // Show first few, transitions, and last few
    let mut last_peer_count: Option<usize> = None;
    for (i, r) in results.iter().enumerate() {
        let elapsed_ms = r.elapsed_since_restart.as_millis();
        let show = i < 3
            || i >= results.len().saturating_sub(2)
            || r.peer_count != last_peer_count;

        if show {
            match (&r.peer_count, &r.error) {
                (Some(count), _) => {
                    println!(
                        "    [{:>6}ms] metrics => {} gossip peers",
                        elapsed_ms, count
                    );
                }
                (_, Some(err)) => {
                    println!("    [{:>6}ms] metrics => ERROR ({})", elapsed_ms, err);
                }
                _ => {}
            }
        }
        last_peer_count = r.peer_count;
    }

    let first_with_peers = results.iter().find(|r| r.peer_count.map_or(false, |c| c > 0));
    if let Some(first) = first_with_peers {
        println!(
            "  => Network Metrics: first peer visible at {}ms",
            first.elapsed_since_restart.as_millis(),
        );
    } else {
        println!("  => Network Metrics: no peers seen during polling window");
    }
}

fn print_full_report(
    test_name: &str,
    local_results: &[PollResult],
    network_results: &[PollResult],
    metrics_results: &[MetricsPollResult],
) {
    let local_first = local_results
        .iter()
        .find(|r| r.link_count.map_or(false, |c| c > 0));
    let network_first = network_results
        .iter()
        .find(|r| r.link_count.map_or(false, |c| c > 0));
    let metrics_first_peer = metrics_results
        .iter()
        .find(|r| r.peer_count.map_or(false, |c| c > 0));

    println!("\n========================================");
    println!("  REPORT: {}", test_name);
    println!("========================================");

    print_results("Local Strategy", local_results);
    print_results("Network Strategy", network_results);
    print_metrics_results(metrics_results);

    println!("\n  --- Summary ---");
    match (local_first, network_first) {
        (Some(l), Some(n)) => {
            println!(
                "    Local   first data: {}ms after restart",
                l.elapsed_since_restart.as_millis()
            );
            println!(
                "    Network first data: {}ms after restart",
                n.elapsed_since_restart.as_millis()
            );
            let diff = n.elapsed_since_restart.as_millis() as i128
                - l.elapsed_since_restart.as_millis() as i128;
            println!(
                "    Network was {}ms {} than local",
                diff.unsigned_abs(),
                if diff > 0 { "slower" } else { "faster" },
            );
        }
        (Some(l), None) => {
            println!(
                "    Local   first data: {}ms after restart",
                l.elapsed_since_restart.as_millis()
            );
            println!("    Network: NEVER returned data (timed out)");
        }
        (None, Some(n)) => {
            println!("    Local: NEVER returned data (timed out)");
            println!(
                "    Network first data: {}ms after restart",
                n.elapsed_since_restart.as_millis()
            );
        }
        (None, None) => {
            println!("    NEITHER strategy returned data (both timed out)");
        }
    }
    if let Some(m) = metrics_first_peer {
        println!(
            "    First gossip peer:  {}ms after restart",
            m.elapsed_since_restart.as_millis()
        );
    } else {
        println!("    First gossip peer:  NEVER seen");
    }
    println!("========================================\n");
}

/// Test 1: After conductor restart, compare local vs network get_links behavior,
/// with concurrent network metrics polling.
///
/// Local strategy should return data quickly (persisted in local DHT).
/// Network strategy may return empty/error initially, then succeed once networking re-establishes.
/// Network metrics show when peers become visible to the gossip layer.
#[tokio::test(flavor = "multi_thread")]
async fn test_restart_get_links_local_vs_network() {
    holochain_trace::test_run();

    let zomes = create_link_zomes();
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    // Phase 1: Set up two conductors, create data, establish consistency.
    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        2,
        SweetConductorConfig::rendezvous(true),
    )
    .await;

    let apps = conductors
        .setup_app("test_app", [&dna_file])
        .await
        .unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();

    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);

    // Create entry and link on Alice.
    let base_hash: ActionHash = conductors[0].call(&alice_zome, "create_entry", ()).await;
    let _link_hash: ActionHash = conductors[0]
        .call(&alice_zome, "create_link", base_hash.clone())
        .await;

    // Set full arcs and exchange peer info.
    conductors[0]
        .declare_full_storage_arcs(alice.dna_hash())
        .await;
    conductors[1]
        .declare_full_storage_arcs(bob.dna_hash())
        .await;
    conductors.exchange_peer_info().await;

    // Wait for consistency so Bob has the data locally.
    await_consistency([&alice, &bob]).await.unwrap();

    // Verify baseline: Bob can see the link with both strategies.
    let baseline_local: Vec<Link> = conductors[1]
        .call(&bob_zome, "get_links_local", base_hash.clone())
        .await;
    assert_eq!(
        baseline_local.len(),
        1,
        "Baseline: Bob should see 1 link via local strategy"
    );

    let baseline_network: Vec<Link> = conductors[1]
        .call(&bob_zome, "get_links_network", base_hash.clone())
        .await;
    assert_eq!(
        baseline_network.len(),
        1,
        "Baseline: Bob should see 1 link via network strategy"
    );

    println!("Baseline verified. Shutting down Bob...");

    let dna_hash = bob.dna_hash().clone();

    // Phase 2: Shutdown and restart Bob.
    conductors[1].shutdown().await;
    println!("Bob shut down. Restarting...");
    conductors[1].startup(false).await;
    let restart_instant = Instant::now();
    println!("Bob restarted. Starting polling loops...");

    // Phase 3: Poll all three concurrently: local, network, and metrics.
    let poll_timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(200);

    let metrics_stop = Arc::new(AtomicBool::new(false));
    let metrics_stop_clone = metrics_stop.clone();

    let ((local_results, network_results), metrics_results) = tokio::join!(
        async {
            let results = tokio::join!(
                poll_get_links(
                    &conductors[1],
                    &bob_zome,
                    "get_links_local",
                    "local",
                    base_hash.clone(),
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
                poll_get_links(
                    &conductors[1],
                    &bob_zome,
                    "get_links_network",
                    "network",
                    base_hash.clone(),
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
            );
            metrics_stop_clone.store(true, Ordering::Relaxed);
            results
        },
        poll_network_metrics(
            &conductors[1],
            &dna_hash,
            restart_instant,
            poll_timeout,
            Duration::from_millis(500),
            metrics_stop,
        ),
    );

    // Phase 4: Report and assert results.
    print_full_report(
        "Restart: Local vs Network",
        &local_results,
        &network_results,
        &metrics_results,
    );

    // Local strategy should succeed (data persisted in local DHT).
    assert!(
        local_results.iter().any(|r| r.link_count == Some(1)),
        "Local strategy should return 1 link after restart (data persisted in DHT)"
    );

    // Network strategy should eventually succeed.
    assert!(
        network_results.iter().any(|r| r.link_count == Some(1)),
        "Network strategy should eventually return 1 link after networking re-establishes"
    );
}

/// Test 2: After restart with an offline peer in bootstrap, compare local vs network,
/// with concurrent network metrics polling.
///
/// Bootstrap returns both an online and an offline peer. Local strategy should work
/// immediately. Network strategy should eventually succeed via the online peer.
/// Network metrics show peer discovery including the offline peer.
#[tokio::test(flavor = "multi_thread")]
async fn test_offline_bootstrap_peer_local_vs_network() {
    holochain_trace::test_run();

    let zomes = create_link_zomes();
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

    // Phase 1: Set up three conductors, create data, establish consistency.
    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        3,
        SweetConductorConfig::rendezvous(true),
    )
    .await;

    let apps = conductors
        .setup_app("test_app", [&dna_file])
        .await
        .unwrap();
    let ((alice,), (bob,), (carol,)) = apps.into_tuples();

    let alice_zome = alice.zome(SweetInlineZomes::COORDINATOR);
    let bob_zome = bob.zome(SweetInlineZomes::COORDINATOR);
    let carol_zome = carol.zome(SweetInlineZomes::COORDINATOR);

    // Create entry and link on Alice.
    let base_hash: ActionHash = conductors[0].call(&alice_zome, "create_entry", ()).await;
    let _link_hash: ActionHash = conductors[0]
        .call(&alice_zome, "create_link", base_hash.clone())
        .await;

    // Set full arcs and exchange peer info for all three.
    for (i, cell) in [&alice, &bob, &carol].iter().enumerate() {
        conductors[i]
            .declare_full_storage_arcs(cell.dna_hash())
            .await;
    }
    conductors.exchange_peer_info().await;

    // Wait for consistency across all three.
    await_consistency([&alice, &bob, &carol]).await.unwrap();

    // Verify baseline: all can see the link.
    let baseline_alice: Vec<Link> = conductors[0]
        .call(&alice_zome, "get_links_local", base_hash.clone())
        .await;
    assert_eq!(baseline_alice.len(), 1, "Baseline: Alice should see 1 link");

    let baseline_bob: Vec<Link> = conductors[1]
        .call(&bob_zome, "get_links_local", base_hash.clone())
        .await;
    assert_eq!(baseline_bob.len(), 1, "Baseline: Bob should see 1 link");

    let baseline_carol: Vec<Link> = conductors[2]
        .call(&carol_zome, "get_links_local", base_hash.clone())
        .await;
    assert_eq!(baseline_carol.len(), 1, "Baseline: Carol should see 1 link");

    println!("Baseline verified for all 3 conductors.");

    let dna_hash = bob.dna_hash().clone();

    // Phase 2: Take Carol permanently offline, then restart Bob.
    // Bootstrap still has Carol's agent info from before she went offline.
    println!("Shutting down Carol (permanently offline)...");
    conductors[2].shutdown().await;

    println!("Shutting down Bob...");
    conductors[1].shutdown().await;

    println!("Restarting Bob (Carol still offline, Alice still online)...");
    conductors[1].startup(false).await;
    let restart_instant = Instant::now();
    println!("Bob restarted. Starting polling loops...");

    // Phase 3: Poll all three concurrently.
    let poll_timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(200);

    let metrics_stop = Arc::new(AtomicBool::new(false));
    let metrics_stop_clone = metrics_stop.clone();

    let ((local_results, network_results), metrics_results) = tokio::join!(
        async {
            let results = tokio::join!(
                poll_get_links(
                    &conductors[1],
                    &bob_zome,
                    "get_links_local",
                    "local",
                    base_hash.clone(),
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
                poll_get_links(
                    &conductors[1],
                    &bob_zome,
                    "get_links_network",
                    "network",
                    base_hash.clone(),
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
            );
            metrics_stop_clone.store(true, Ordering::Relaxed);
            results
        },
        poll_network_metrics(
            &conductors[1],
            &dna_hash,
            restart_instant,
            poll_timeout,
            Duration::from_millis(500),
            metrics_stop,
        ),
    );

    // Phase 4: Report and assert results.
    print_full_report(
        "Offline Peer: Local vs Network",
        &local_results,
        &network_results,
        &metrics_results,
    );

    // Local strategy should succeed quickly (data persisted, no network needed).
    assert!(
        local_results.iter().any(|r| r.link_count == Some(1)),
        "Local strategy should return 1 link (unaffected by offline peer)"
    );

    // Network strategy should eventually succeed via Alice, despite Carol being offline.
    assert!(
        network_results.iter().any(|r| r.link_count == Some(1)),
        "Network strategy should eventually return 1 link via online peer (Alice)"
    );
}
