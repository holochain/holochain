//! Websocket-based variants of the startup blocking tests.
//!
//! These tests make zome calls through the app websocket interface (like a real
//! client app would) rather than through direct Rust method calls.

#![cfg(feature = "test_utils")]

use super::{create_link_zomes, PollResult};
use crate::tests::test_utils::{call_zome_fn_fallible, grant_zome_call_capability};
use ed25519_dalek::SigningKey;
use holochain::sweettest::*;
use holochain_conductor_api::AppResponse;
use holochain_types::prelude::*;
use holochain_websocket::WebsocketSender;
use holochain_zome_types::link::Link;
use rand_dalek::rngs::OsRng;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Set up a websocket client for a conductor + app, returning the sender,
/// signing keypair, cap secrets (local, network), and the WsPollRecv (which must be kept alive).
async fn setup_app_ws(
    conductor: &SweetConductor,
    cell_id: &CellId,
    app_id: &str,
) -> (WebsocketSender, SigningKey, CapSecret, CapSecret, WsPollRecv) {
    let (app_tx, app_poll) =
        conductor.app_ws_client::<AppResponse>(app_id.into()).await;

    // Generate a signing keypair and grant zome call capability
    let mut rng = OsRng;
    let signing_keypair = SigningKey::generate(&mut rng);
    let signing_key =
        AgentPubKey::from_raw_32(signing_keypair.verifying_key().as_bytes().to_vec());

    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let (mut admin_tx, admin_rx) = websocket_client_by_port(admin_port).await.unwrap();
    let _admin_rx = holochain::sweettest::WsPollRecv::new::<holochain_conductor_api::AdminResponse>(admin_rx);

    // Each grant creates its own cap secret, so we need separate secrets per function.
    let cap_secret_local = grant_zome_call_capability(
        &mut admin_tx,
        cell_id,
        SweetInlineZomes::COORDINATOR.into(),
        "get_links_local".into(),
        signing_key.clone(),
    )
    .await
    .unwrap();

    let cap_secret_network = grant_zome_call_capability(
        &mut admin_tx,
        cell_id,
        SweetInlineZomes::COORDINATOR.into(),
        "get_links_network".into(),
        signing_key,
    )
    .await
    .unwrap();

    (app_tx, signing_keypair, cap_secret_local, cap_secret_network, app_poll)
}

/// Poll get_links via websocket repeatedly until we get data or timeout.
async fn poll_get_links_ws(
    app_tx: &WebsocketSender,
    cell_id: &CellId,
    signing_keypair: &SigningKey,
    cap_secret: CapSecret,
    fn_name: &str,
    strategy_label: &'static str,
    base: &ActionHash,
    restart_instant: Instant,
    timeout: Duration,
    interval: Duration,
) -> Vec<PollResult> {
    let mut results = Vec::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let elapsed = restart_instant.elapsed();

        let response = call_zome_fn_fallible(
            app_tx,
            cell_id.clone(),
            signing_keypair,
            cap_secret,
            SweetInlineZomes::COORDINATOR.into(),
            fn_name.into(),
            base,
        )
        .await;

        let (link_count, error) = match response {
            AppResponse::ZomeCalled(output) => {
                match ExternIO::decode::<Vec<Link>>(&output) {
                    Ok(links) => (Some(links.len()), None),
                    Err(e) => (None, Some(format!("decode error: {:?}", e))),
                }
            }
            AppResponse::Error(e) => (None, Some(format!("{:?}", e))),
            other => (None, Some(format!("unexpected response: {:?}", other))),
        };

        let has_data = link_count.map_or(false, |c| c > 0);

        results.push(PollResult {
            elapsed_since_restart: elapsed,
            strategy: strategy_label,
            link_count,
            error,
        });

        if has_data {
            for _ in 0..3 {
                tokio::time::sleep(interval).await;
                let elapsed = restart_instant.elapsed();
                let response = call_zome_fn_fallible(
                    app_tx,
                    cell_id.clone(),
                    signing_keypair,
                    cap_secret,
                    SweetInlineZomes::COORDINATOR.into(),
                    fn_name.into(),
                    base,
                )
                .await;
                let (link_count, error) = match response {
                    AppResponse::ZomeCalled(output) => {
                        match ExternIO::decode::<Vec<Link>>(&output) {
                            Ok(links) => (Some(links.len()), None),
                            Err(e) => (None, Some(format!("decode error: {:?}", e))),
                        }
                    }
                    AppResponse::Error(e) => (None, Some(format!("{:?}", e))),
                    other => (None, Some(format!("unexpected response: {:?}", other))),
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

/// Test 1 (websocket): After conductor restart, compare local vs network get_links
/// through the app websocket interface.
#[tokio::test(flavor = "multi_thread")]
async fn test_restart_get_links_local_vs_network_ws() {
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
    let bob_cell_id = bob.cell_id().clone();

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
    await_consistency([&alice, &bob]).await.unwrap();

    // Verify baseline via direct call.
    let baseline: Vec<Link> = conductors[1]
        .call(&bob_zome, "get_links_local", base_hash.clone())
        .await;
    assert_eq!(baseline.len(), 1, "Baseline: Bob should see 1 link");

    println!("[WS] Baseline verified. Shutting down Bob...");

    let dna_hash = bob.dna_hash().clone();

    // Phase 2: Shutdown and restart Bob.
    conductors[1].shutdown().await;
    println!("[WS] Bob shut down. Restarting...");
    conductors[1].startup(false).await;

    // Set up websocket client after restart.
    let (app_tx, signing_keypair, cap_local, cap_network, _app_poll) =
        setup_app_ws(&conductors[1], &bob_cell_id, "test_app").await;

    let restart_instant = Instant::now();
    println!("[WS] Bob restarted, websocket connected. Starting polling loops...");

    // Phase 3: Poll both strategies concurrently via websocket + metrics.
    let poll_timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(200);

    let metrics_stop = Arc::new(AtomicBool::new(false));
    let metrics_stop_clone = metrics_stop.clone();

    let ((local_results, network_results), metrics_results) = tokio::join!(
        async {
            let results = tokio::join!(
                poll_get_links_ws(
                    &app_tx,
                    &bob_cell_id,
                    &signing_keypair,
                    cap_local,
                    "get_links_local",
                    "local",
                    &base_hash,
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
                poll_get_links_ws(
                    &app_tx,
                    &bob_cell_id,
                    &signing_keypair,
                    cap_network,
                    "get_links_network",
                    "network",
                    &base_hash,
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
            );
            metrics_stop_clone.store(true, Ordering::Relaxed);
            results
        },
        super::poll_network_metrics(
            &conductors[1],
            &dna_hash,
            restart_instant,
            poll_timeout,
            Duration::from_millis(500),
            metrics_stop,
        ),
    );

    // Phase 4: Report and assert results.
    super::print_full_report(
        "Restart via Websocket: Local vs Network",
        &local_results,
        &network_results,
        &metrics_results,
    );

    assert!(
        local_results.iter().any(|r| r.link_count == Some(1)),
        "[WS] Local strategy should return 1 link after restart"
    );

    assert!(
        network_results.iter().any(|r| r.link_count == Some(1)),
        "[WS] Network strategy should eventually return 1 link"
    );
}

/// Test 2 (websocket): After restart with an offline peer in bootstrap, compare
/// local vs network through the app websocket interface.
#[tokio::test(flavor = "multi_thread")]
async fn test_offline_bootstrap_peer_local_vs_network_ws() {
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
    let bob_cell_id = bob.cell_id().clone();

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
    await_consistency([&alice, &bob, &carol]).await.unwrap();

    // Verify baseline.
    let baseline: Vec<Link> = conductors[1]
        .call(&bob.zome(SweetInlineZomes::COORDINATOR), "get_links_local", base_hash.clone())
        .await;
    assert_eq!(baseline.len(), 1, "Baseline: Bob should see 1 link");

    println!("[WS] Baseline verified for all 3 conductors.");

    let dna_hash = bob.dna_hash().clone();

    // Phase 2: Take Carol permanently offline, then restart Bob.
    println!("[WS] Shutting down Carol (permanently offline)...");
    conductors[2].shutdown().await;

    println!("[WS] Shutting down Bob...");
    conductors[1].shutdown().await;

    println!("[WS] Restarting Bob (Carol still offline, Alice still online)...");
    conductors[1].startup(false).await;

    // Set up websocket client after restart.
    let (app_tx, signing_keypair, cap_local, cap_network, _app_poll) =
        setup_app_ws(&conductors[1], &bob_cell_id, "test_app").await;

    let restart_instant = Instant::now();
    println!("[WS] Bob restarted, websocket connected. Starting polling loops...");

    // Phase 3: Poll all three concurrently via websocket.
    let poll_timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(200);

    let metrics_stop = Arc::new(AtomicBool::new(false));
    let metrics_stop_clone = metrics_stop.clone();

    let ((local_results, network_results), metrics_results) = tokio::join!(
        async {
            let results = tokio::join!(
                poll_get_links_ws(
                    &app_tx,
                    &bob_cell_id,
                    &signing_keypair,
                    cap_local,
                    "get_links_local",
                    "local",
                    &base_hash,
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
                poll_get_links_ws(
                    &app_tx,
                    &bob_cell_id,
                    &signing_keypair,
                    cap_network,
                    "get_links_network",
                    "network",
                    &base_hash,
                    restart_instant,
                    poll_timeout,
                    poll_interval,
                ),
            );
            metrics_stop_clone.store(true, Ordering::Relaxed);
            results
        },
        super::poll_network_metrics(
            &conductors[1],
            &dna_hash,
            restart_instant,
            poll_timeout,
            Duration::from_millis(500),
            metrics_stop,
        ),
    );

    // Phase 4: Report and assert results.
    super::print_full_report(
        "Offline Peer via Websocket: Local vs Network",
        &local_results,
        &network_results,
        &metrics_results,
    );

    assert!(
        local_results.iter().any(|r| r.link_count == Some(1)),
        "[WS] Local strategy should return 1 link (unaffected by offline peer)"
    );

    assert!(
        network_results.iter().any(|r| r.link_count == Some(1)),
        "[WS] Network strategy should eventually return 1 link via online peer (Alice)"
    );
}
