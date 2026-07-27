#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::single_match)]

use hdk::prelude::*;
use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::SweetInlineZomes;
use holochain::sweettest::{await_consistency, SweetConductor, SweetDnaFile};
use holochain::sweettest::{SweetConductorBatch, SweetLocalRendezvous};
use holochain::test_utils::inline_zomes::simple_crud_zome;
use holochain_zome_types::prelude::Record;

/// Test that conductors with arcs clamped to zero do not gossip.
#[tokio::test(flavor = "multi_thread")]
async fn get_with_zero_arc_2_way() {
    holochain_trace::test_run();

    // Standard config with arc clamped to zero and publishing off
    let empty_arc_conductor_config =
        SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
            nc.disable_publish = true;
            nc.target_arc_factor = 0;
        });
    let standard_config = SweetConductorConfig::rendezvous(false);
    let mut conductors =
        SweetConductorBatch::from_configs_rendezvous([standard_config, empty_arc_conductor_config])
            .await;

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
    let config = SweetConductorConfig::rendezvous(false)
        .tune_network_config(|nc| nc.disable_bootstrap = true);
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

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
    await_consistency([&cell_0, &cell_1]).await.unwrap();
    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

/// Test that when a new conductor joins, gossip picks up existing data without needing a publish.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn new_conductor_reaches_consistency_with_existing_conductor() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mut conductor0 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let app0 = conductor0.setup_app("app", [&dna_file]).await.unwrap();
    let cell0 = app0.into_cells().pop().unwrap();
    let zome0 = cell0.zome(SweetInlineZomes::COORDINATOR);

    // Create an entry before the conductors know about each other
    let hash: ActionHash = conductor0
        .call(&zome0, "create_string", "hi".to_string())
        .await;

    // Startup and do peer discovery
    let mut conductor1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        conductor0.rendezvous().unwrap().clone(),
    )
    .await;
    let app1 = conductor1.setup_app("app", [&dna_file]).await.unwrap();
    let cell1 = app1.into_cells().pop().unwrap();
    let zome1 = cell1.zome(SweetInlineZomes::COORDINATOR);

    SweetConductor::exchange_peer_info([&conductor0, &conductor1]).await;

    await_consistency([&cell0, &cell1]).await.unwrap();
    let record: Option<Record> = conductor1.call(&zome1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

/// A conductor joining after private entries were authored on the existing
/// chain must reach consistency via gossip alone: the private CreateEntry
/// ops are withheld from advertisement and serving, while all public ops
/// for the same records transfer normally (#5871).
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn new_conductor_syncs_via_gossip_with_private_entries() {
    holochain_trace::test_run();
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mut conductor0 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let app0 = conductor0.setup_app("app", [&dna_file]).await.unwrap();
    let cell0 = app0.into_cells().pop().unwrap();
    let zome0 = cell0.zome(SweetInlineZomes::COORDINATOR);

    // Author public and private entries before conductor1 exists.
    let public_hash0: ActionHash = conductor0
        .call(&zome0, "create_string", "hi".to_string())
        .await;
    let (_priv_action_hash, priv_entry_hash): (ActionHash, EntryHash) = conductor0
        .call(&zome0, "create_priv_string", "secret".to_string())
        .await;

    let mut conductor1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        conductor0.rendezvous().unwrap().clone(),
    )
    .await;
    let app1 = conductor1.setup_app("app", [&dna_file]).await.unwrap();
    let cell1 = app1.into_cells().pop().unwrap();
    let zome1 = cell1.zome(SweetInlineZomes::COORDINATOR);

    SweetConductor::exchange_peer_info([&conductor0, &conductor1]).await;
    await_consistency([&cell0, &cell1]).await.unwrap();

    // The public record is available to the new conductor.
    let record: Option<Record> = conductor1.call(&zome1, "read", public_hash0.clone()).await;
    assert_eq!(record.unwrap().action_address(), &public_hash0);

    // The private entry's basis has no authority: its CreateEntry op is
    // withheld, so a get by entry hash finds nothing.
    let records: Vec<Option<Record>> = conductor1
        .call(&zome1, "read_entry", priv_entry_hash.clone())
        .await;
    assert!(records.into_iter().flatten().next().is_none());

    // Reverse direction with private entries on both chains: conductor1
    // authors while conductor0 is offline, so publish cannot deliver and
    // gossip must once conductor0 returns.
    conductor0.shutdown().await;
    let public_hash1: ActionHash = conductor1
        .call(&zome1, "create_string", "hello".to_string())
        .await;
    let (_a, _e): (ActionHash, EntryHash) = conductor1
        .call(&zome1, "create_priv_string", "also secret".to_string())
        .await;
    conductor0.startup().await;
    SweetConductor::exchange_peer_info([&conductor0, &conductor1]).await;
    await_consistency([&cell0, &cell1]).await.unwrap();

    let record: Option<Record> = conductor0.call(&zome0, "read", public_hash1.clone()).await;
    assert_eq!(record.unwrap().action_address(), &public_hash1);
}
