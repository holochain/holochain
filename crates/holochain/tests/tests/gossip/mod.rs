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
    // Disable publishing on both conductors so gossip is the only possible
    // transport, rather than relying on the publish interval to exceed the
    // consistency window.
    let config =
        SweetConductorConfig::rendezvous(true).tune_network_config(|nc| nc.disable_publish = true);
    let mut conductor0 =
        SweetConductor::from_config_rendezvous(config.clone(), SweetLocalRendezvous::new().await)
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

    let mut conductor1 =
        SweetConductor::from_config_rendezvous(config, conductor0.rendezvous().unwrap().clone())
            .await;
    let app1 = conductor1.setup_app("app", [&dna_file]).await.unwrap();
    let cell1 = app1.into_cells().pop().unwrap();
    let zome1 = cell1.zome(SweetInlineZomes::COORDINATOR);

    SweetConductor::exchange_peer_info([&conductor0, &conductor1]).await;
    await_consistency([&cell0, &cell1]).await.unwrap();

    // The public record is available to the new conductor.
    let record: Option<Record> = conductor1.call(&zome1, "read", public_hash0.clone()).await;
    assert_eq!(record.unwrap().action_address(), &public_hash0);

    // Consistency has been reached, so the get finding nothing means the
    // private entry's CreateEntry op is withheld from gossip: no authority
    // holds the entry.
    let records: Vec<Option<Record>> = conductor1
        .call(&zome1, "read_entry", priv_entry_hash.clone())
        .await;
    assert!(records.into_iter().flatten().next().is_none());

    // Reverse direction with private entries on both chains: conductor1
    // authors its own public and private entries and gossip carries the
    // public data back to conductor0.
    let public_hash1: ActionHash = conductor1
        .call(&zome1, "create_string", "hello".to_string())
        .await;
    let (_priv_action_hash1, priv_entry_hash1): (ActionHash, EntryHash) = conductor1
        .call(&zome1, "create_priv_string", "also secret".to_string())
        .await;
    await_consistency([&cell0, &cell1]).await.unwrap();

    let record: Option<Record> = conductor0.call(&zome0, "read", public_hash1.clone()).await;
    assert_eq!(record.unwrap().action_address(), &public_hash1);

    let records: Vec<Option<Record>> = conductor0
        .call(&zome0, "read_entry", priv_entry_hash1.clone())
        .await;
    assert!(records.into_iter().flatten().next().is_none());
}

/// Every op hash the kitsune2 op store advertises must be servable while
/// private entries sit on the chain. An advertised-but-unservable op is the
/// #5871 failure class: peers request it forever, op stores never converge,
/// and storage arcs never grow.
#[tokio::test(flavor = "multi_thread")]
async fn advertised_ops_are_servable_with_private_entries_on_chain() {
    use holochain_p2p::HolochainOpStore;
    use holochain_types::op::{produce_ops_from_record, ChainOp, DhtOp, OpEntry};
    use holochain_zome_types::prelude::ChainOpType;
    use kitsune2_api::{DhtArc, OpId, OpStore};
    use std::collections::HashSet;
    use std::time::Duration;

    holochain_trace::test_run();

    let mut conductor = SweetConductor::standard().await;
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let app = conductor.setup_app("app", [&dna_file]).await.unwrap();
    let cell = app.into_cells().pop().unwrap();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);

    const PRIVATE_CONTENT: &str = "private-entry-content-sentinel";

    let public_hash: ActionHash = conductor
        .call(&zome, "create_string", "public-entry-content".to_string())
        .await;
    let (priv_action_hash, _priv_entry_hash): (ActionHash, EntryHash) = conductor
        .call(&zome, "create_priv_string", PRIVATE_CONTENT.to_string())
        .await;

    // The author's own get returns the private record with its entry, which
    // is what `produce_ops_from_record` needs to compute the full op set.
    let public_record: Option<Record> = conductor.call(&zome, "read", public_hash.clone()).await;
    let private_record: Option<Record> = conductor
        .call(&zome, "read", priv_action_hash.clone())
        .await;
    let private_record = private_record.unwrap();
    assert!(private_record.entry().as_option().is_some());

    let priv_ops = produce_ops_from_record(&private_record);
    let priv_create_entry = priv_ops
        .iter()
        .find(|o| o.op_type == ChainOpType::CreateEntry)
        .expect("a private create must still produce a CreateEntry op locally");
    let priv_create_entry_id = priv_create_entry
        .op_hash
        .to_located_k2_op_id(&priv_create_entry.basis_hash);
    let priv_create_entry_raw = priv_create_entry.op_hash.get_raw_36().to_vec();

    let expected_servable: HashSet<OpId> = produce_ops_from_record(&public_record.unwrap())
        .iter()
        .chain(
            priv_ops
                .iter()
                .filter(|o| o.op_type != ChainOpType::CreateEntry),
        )
        .map(|o| o.op_hash.to_located_k2_op_id(&o.basis_hash))
        .collect();

    let op_store = HolochainOpStore::new(
        cell.dht_store().clone(),
        dna_file.dna_hash().clone(),
        std::sync::Arc::new(std::sync::OnceLock::new()),
    );

    // Wait until the authored ops are advertised.
    let advertised: Vec<OpId> = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (ids, _total_size) = op_store
                .retrieve_op_hashes_in_time_slice(
                    DhtArc::FULL,
                    kitsune2_api::Timestamp::from_micros(0),
                    kitsune2_api::Timestamp::from_micros(i64::MAX),
                )
                .await
                .unwrap();
            let set: HashSet<OpId> = ids.iter().cloned().collect();
            if expected_servable.is_subset(&set) {
                break ids;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await
    .expect("timed out waiting for authored ops to become servable");

    // The private CreateEntry op is stored locally for its author, so this
    // test is not passing vacuously.
    let held = cell
        .dht_store()
        .as_read()
        .check_op_hashes_present(&[priv_create_entry_raw])
        .await
        .unwrap();
    assert_eq!(
        held.len(),
        1,
        "private CreateEntry op must be stored locally"
    );

    // ...but it is never advertised.
    let advertised_set: HashSet<OpId> = advertised.iter().cloned().collect();
    assert!(
        !advertised_set.contains(&priv_create_entry_id),
        "private CreateEntry op must not be advertised"
    );

    // Everything advertised must be servable.
    let served = op_store.retrieve_ops(advertised.clone()).await.unwrap();
    let served_ids: HashSet<OpId> = served.iter().map(|op| op.op_id.clone()).collect();
    assert_eq!(
        advertised_set, served_ids,
        "every advertised op must be servable"
    );

    // No served op carries the private entry content, and every served
    // CreateEntry op carries its entry.
    let sentinel = PRIVATE_CONTENT.as_bytes();
    let mut served_create_entry_count = 0;
    for meta_op in &served {
        assert!(
            !meta_op
                .op_data
                .windows(sentinel.len())
                .any(|w| w == sentinel),
            "private entry content leaked into a served op"
        );
        let op: DhtOp = holochain_serialized_bytes::prelude::decode(&meta_op.op_data).unwrap();
        if let DhtOp::ChainOp(chain_op) = &op {
            if let ChainOp::CreateEntry(_, entry) = &**chain_op {
                assert!(
                    matches!(entry, OpEntry::Present(_)),
                    "served CreateEntry op is missing its entry"
                );
                served_create_entry_count += 1;
            }
        }
    }
    assert!(served_create_entry_count >= 1);
}
