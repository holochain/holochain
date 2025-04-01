#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::single_match)]

use hdk::prelude::*;
use holo_hash::ActionHash;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::SweetInlineZomes;
use holochain::sweettest::{await_consistency, SweetConductor, SweetDnaFile};
use holochain::test_utils::inline_zomes::simple_crud_zome;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::NetworkConfig;
use holochain_zome_types::record::Record;

/// Test that conductors with arcs clamped to zero do not gossip.
#[tokio::test(flavor = "multi_thread")]
async fn get_with_zero_arc_2_way() {
    holochain_trace::test_run();

    // Standard config with arc clamped to zero and publishing off
    let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    conductors[0]
        .declare_full_storage_arcs(dna_file.dna_hash())
        .await;
    conductors.exchange_peer_info().await;

    let zome_0 = alice.zome(SweetInlineZomes::COORDINATOR);
    let hash_0: ActionHash = conductors[0]
        .call(&zome_0, "create_string", "hi".to_string())
        .await;

    let zome_1 = bob.zome(SweetInlineZomes::COORDINATOR);
    let hash_1: ActionHash = conductors[1]
        .call(&zome_1, "create_string", "hi".to_string())
        .await;

    // can't await consistency because one node is neither publishing nor gossiping, and is relying only on `get`

    let record_0: Option<Record> = conductors[0].call(&zome_0, "read", hash_1.clone()).await;
    let record_1: Option<Record> = conductors[1].call(&zome_1, "read", hash_0.clone()).await;

    // 1 is not a valid target for the get, and 0 did not publish, so 0 can't get 1's data.
    assert!(record_0.is_none());

    // 1 can get 0's data, though.
    assert!(record_1.is_some());
}

/// Test that when the conductor shuts down, gossip does not continue,
/// and when it restarts, gossip resumes.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn gossip_resumes_after_restart() {
    holochain_trace::test_run();
    let mut conductors = SweetConductorBatch::from_config(
        2,
        ConductorConfig {
            network: NetworkConfig {
                mem_bootstrap: false,
                ..Default::default()
            }
            .with_gossip_initiate_interval_ms(1_000)
            .with_gossip_min_initiate_interval_ms(750),
            ..Default::default()
        },
    )
    .await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let ((cell_0,), (cell_1,)) = apps.into_tuples();
    let zome_0 = cell_0.zome(SweetInlineZomes::COORDINATOR);
    let zome_1 = cell_1.zome(SweetInlineZomes::COORDINATOR);

    // Create an entry before the conductors know about each other
    let hash: ActionHash = conductors[0]
        .call(&zome_0, "create_string", "hi".to_string())
        .await;

    conductors[0].shutdown().await;

    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert!(record.is_none());

    conductors[0].startup().await;
    conductors.exchange_peer_info().await;

    // Ensure that gossip loops resume upon startup.
    await_consistency(30, [&cell_0, &cell_1]).await.unwrap();
    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

/// Test that when a new conductor joins, gossip picks up existing data without needing a publish.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn new_conductor_reaches_consistency_with_existing_conductor() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mk_conductor = || async {
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
        let cell = app.into_cells().pop().unwrap();
        let zome = cell.zome(SweetInlineZomes::COORDINATOR);
        (conductor, cell, zome)
    };
    let (conductor0, cell0, zome0) = mk_conductor().await;

    // Create an entry before the conductors know about each other
    let hash: ActionHash = conductor0
        .call(&zome0, "create_string", "hi".to_string())
        .await;

    // Startup and do peer discovery
    let (conductor1, cell1, zome1) = mk_conductor().await;

    await_consistency(30, [&cell0, &cell1]).await.unwrap();
    let record: Option<Record> = conductor1.call(&zome1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}
