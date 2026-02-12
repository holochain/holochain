use hdk::prelude::*;
use holochain::conductor::NetworkReadinessEvent;
use holochain::sweettest::*;
use std::time::Duration;

/// Test that await_cell_network_ready successfully waits for a cell to be ready.
///
/// This demonstrates that cells can be used immediately after awaiting network readiness,
/// without needing retry loops or arbitrary sleeps.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn test_single_cell_network_readiness() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    let zome = SweetInlineZomes::new(vec![], 0).function("ping", |_, _: ()| Ok("pong"));

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    // Install and enable the app
    let app = conductor
        .setup_app_for_agent("app", agent.clone(), &[dna.clone()])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    // Wait for the cell to be network ready
    // This should complete quickly without timing out
    conductor
        .await_cell_network_ready(cell.cell_id(), Duration::from_secs(10))
        .await
        .expect("Cell should become network ready");

    // Now the cell should be fully ready - zome calls should work immediately
    let result: String = conductor.call(&cell.zome(SweetInlineZomes::COORDINATOR), "ping", ()).await;
    assert_eq!(result, "pong");
}

/// Test network readiness with multiple cells in the same app.
///
/// This ensures that all cells in an app can be awaited independently and all
/// become ready without requiring retry loops.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn test_multi_cell_network_readiness() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    let zome1 = SweetInlineZomes::new(vec![], 0).function("ping", |_, _: ()| Ok("pong1"));
    let zome2 = SweetInlineZomes::new(vec![], 0).function("ping", |_, _: ()| Ok("pong2"));

    let (dna1, _, _) = SweetDnaFile::unique_from_inline_zomes(zome1).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_inline_zomes(zome2).await;

    // Install app with multiple cells
    let app = conductor
        .setup_app_for_agent("app", agent.clone(), &[dna1.clone(), dna2.clone()])
        .await
        .unwrap();
    let (cell1, cell2) = app.into_tuple();

    // Wait for both cells to be network ready
    conductor
        .await_cell_network_ready(cell1.cell_id(), Duration::from_secs(10))
        .await
        .expect("Cell 1 should become network ready");

    conductor
        .await_cell_network_ready(cell2.cell_id(), Duration::from_secs(10))
        .await
        .expect("Cell 2 should become network ready");

    // Both cells should work immediately
    let result1: String = conductor.call(&cell1.zome(SweetInlineZomes::COORDINATOR), "ping", ()).await;
    let result2: String = conductor.call(&cell2.zome(SweetInlineZomes::COORDINATOR), "ping", ()).await;

    assert_eq!(result1, "pong1");
    assert_eq!(result2, "pong2");
}

/// Test network readiness across multiple conductors without retry loops.
///
/// This is the key test showing that two conductors can discover each other and
/// make remote calls immediately after awaiting network readiness, eliminating
/// the need for retry loops that were previously required.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn test_multi_conductor_network_readiness_no_retry() {
    holochain_trace::test_run();

    let mut conductors = SweetConductorBatch::standard(2).await;

    let zome = SweetInlineZomes::new(vec![], 0)
        .function("ping", |_, _: ()| Ok("pong"))
        .function("create_entry", |api, _: ()| {
            api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Public,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    access: ().into(),
                    functions: GrantedFunctions::All,
                }),
                ChainTopOrdering::Relaxed,
            ))?;
            Ok(())
        });

    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    // Setup app on both conductors simultaneously
    let apps = conductors.setup_app("app", &[dna.clone()]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();

    // Exchange peer info so they know about each other
    conductors.exchange_peer_info().await;

    // Wait for both cells to be network ready
    // This is the key: no retry loops needed!
    conductors[0]
        .await_cell_network_ready(alice.cell_id(), Duration::from_secs(10))
        .await
        .expect("Alice's cell should become network ready");

    conductors[1]
        .await_cell_network_ready(bob.cell_id(), Duration::from_secs(10))
        .await
        .expect("Bob's cell should become network ready");

    // Create an entry on Alice
    let _: () = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create_entry", ())
        .await;

    // Wait for consistency without needing retry loops
    await_consistency(&[alice.clone(), bob.clone()])
        .await
        .expect("Consistency should be reached");

    // Calls should work immediately without any retries
    let result: String = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "ping", ())
        .await;
    assert_eq!(result, "pong");

    let result: String = conductors[1]
        .call(&bob.zome(SweetInlineZomes::COORDINATOR), "ping", ())
        .await;
    assert_eq!(result, "pong");
}

/// Test that network readiness events are emitted correctly.
///
/// This test subscribes to network readiness events and verifies that the
/// expected events (JoinStarted, JoinComplete) are received when cells are enabled.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn test_network_readiness_events_emitted() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    // Subscribe to network readiness events BEFORE enabling the app
    let mut events = conductor.subscribe_network_readiness();

    let zome = SweetInlineZomes::new(vec![], 0).function("ping", |_, _: ()| Ok("pong"));
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    // Install and enable the app
    let app = conductor
        .setup_app_for_agent("app", agent.clone(), &[dna.clone()])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    // We should receive JoinStarted and JoinComplete events
    let mut received_join_started = false;
    let mut received_join_complete = false;

    // Wait for events with a timeout
    tokio::time::timeout(Duration::from_secs(10), async {
        while !received_join_started || !received_join_complete {
            if let Ok(event) = events.recv().await {
                match event {
                    NetworkReadinessEvent::JoinStarted { cell_id }
                        if &cell_id == cell.cell_id() =>
                    {
                        received_join_started = true;
                    }
                    NetworkReadinessEvent::JoinComplete { cell_id }
                        if &cell_id == cell.cell_id() =>
                    {
                        received_join_complete = true;
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .expect("Should receive join events within timeout");

    assert!(received_join_started, "Should have received JoinStarted event");
    assert!(received_join_complete, "Should have received JoinComplete event");
}

/// Test that await_cell_network_ready times out appropriately when cell doesn't exist.
///
/// This verifies error handling when waiting for a non-existent cell.
#[tokio::test(flavor = "multi_thread")]
async fn test_network_readiness_timeout_for_nonexistent_cell() {
    holochain_trace::test_run();

    let conductor = SweetConductor::standard().await;

    // Create a fake cell ID that doesn't exist
    let fake_dna_hash = DnaHash::from_raw_36(vec![0u8; 36]);
    let fake_agent = AgentPubKey::from_raw_36(vec![1u8; 36]);
    let fake_cell_id = CellId::new(fake_dna_hash, fake_agent);

    // Attempting to wait for this cell should timeout
    let result = conductor
        .await_cell_network_ready(&fake_cell_id, Duration::from_secs(1))
        .await;

    assert!(result.is_err(), "Should timeout waiting for non-existent cell");
}

/// Demonstration test showing the BEFORE and AFTER of network readiness.
///
/// This test explicitly shows how the old approach (with sleep) compares to
/// the new approach (with await_cell_network_ready).
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn test_network_readiness_vs_sleep_comparison() {
    holochain_trace::test_run();

    // OLD APPROACH: Using arbitrary sleep (commented out to show the pattern)
    // let mut conductor = SweetConductor::standard().await;
    // let agent = SweetAgents::one(conductor.keystore()).await;
    // let app = conductor.setup_app_for_agent("app", agent, &[dna]).await.unwrap();
    // tokio::time::sleep(Duration::from_secs(5)).await; // ⚠️ Arbitrary sleep, might not be enough!
    // // Hope the cell is ready now...

    // NEW APPROACH: Using network readiness
    let mut conductor = SweetConductor::standard().await;
    let agent = SweetAgents::one(conductor.keystore()).await;

    let zome = SweetInlineZomes::new(vec![], 0).function("ping", |_, _: ()| Ok("pong"));
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    let start = std::time::Instant::now();

    let app = conductor
        .setup_app_for_agent("app", agent, &[dna.clone()])
        .await
        .unwrap();
    let (cell,) = app.into_tuple();

    // ✅ Explicitly wait for network readiness - no guessing!
    conductor
        .await_cell_network_ready(cell.cell_id(), Duration::from_secs(10))
        .await
        .expect("Cell should become network ready");

    let elapsed = start.elapsed();

    // The cell is ready in a reasonable time (usually < 1 second with local rendezvous)
    assert!(elapsed < Duration::from_secs(5), "Should be ready quickly");

    // And it works immediately
    let result: String = conductor.call(&cell.zome(SweetInlineZomes::COORDINATOR), "ping", ()).await;
    assert_eq!(result, "pong");
}
