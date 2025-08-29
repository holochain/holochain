#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::single_match)]

use hdk::prelude::*;
use holo_hash::ActionHash;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::SweetInlineZomes;
use holochain::sweettest::{await_consistency, SweetConductor, SweetDnaFile};
use holochain::test_utils::inline_zomes::simple_crud_zome;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::NetworkConfig;
use holochain_zome_types::record::Record;

#[cfg(feature = "unstable-warrants")]
use {
    holochain::prelude::InlineZomeSet,
    holochain_state::query::CascadeTxnWrapper,
    holochain_state::query::Store,
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

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

    conductors[0].startup(false).await;
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

// Test that warrants are gossiped and received.
// Alice, Bob and Carol start a network and sync. Carol goes offline.
// Alice creates an invalid op, Bob receives it and issues a warrant.
// Carol has warrant issuance disabled and receives the warrant from Bob
// via gossip after she comes back online.
// Publish is disabled for this test.
#[cfg(feature = "unstable-warrants")]
#[tokio::test(flavor = "multi_thread")]
async fn warrant_is_gossiped() {
    holochain_trace::test_run();

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct AppString(String);

    let string_entry_def = EntryDef::default_from_id("string");
    let zome_common = SweetInlineZomes::new(vec![string_entry_def], 0).function(
        "create_string",
        move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        },
    );

    let zome_without_validation = zome_common
        .clone()
        .integrity_function("validate", move |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        });
    // Any action after the genesis actions is invalid.
    let zome_with_validation =
        zome_common
            .clone()
            .integrity_function("validate", move |_api, op: Op| {
                if op.action_seq() > 3 {
                    Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                } else {
                    Ok(ValidateCallbackResult::Valid)
                }
            });

    let network_seed = "seed".to_string();

    let (dna_without_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_without_validation).await;
    let (dna_with_validation, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_with_validation).await;
    assert_eq!(
        dna_without_validation.dna_hash(),
        dna_with_validation.dna_hash()
    );
    let dna_hash = dna_without_validation.dna_hash();

    let config =
        SweetConductorConfig::rendezvous(true).tune_network_config(|nc| nc.disable_publish = true);
    // Disable warrants on Carol's conductor, so that she doesn't issue warrants herself
    // but receives them from Bob.
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true)
        .tune_network_config(|nc| nc.disable_publish = true);
    let mut conductors = SweetConductorBatch::from_configs_rendezvous([
        config.clone(),
        config,
        config_no_warranting,
    ])
    .await;
    let (alice,) = conductors[0]
        .setup_app("test_app", [&dna_without_validation])
        .await
        .unwrap()
        .into_tuple();
    let (bob,) = conductors[1]
        .setup_app("test_app", [&dna_with_validation])
        .await
        .unwrap()
        .into_tuple();
    let (carol,) = conductors[2]
        .setup_app("test_app", [&dna_with_validation])
        .await
        .unwrap()
        .into_tuple();

    println!("AGENTS");
    println!(
        "0 alice {} url {:?}",
        alice.agent_pubkey(),
        conductors[0].dump_network_stats().await.unwrap().peer_urls[0]
    );
    println!(
        "1 bob   {} url {:?}",
        bob.agent_pubkey(),
        conductors[1].dump_network_stats().await.unwrap().peer_urls[0]
    );
    println!(
        "2 carol {} url {:?}",
        carol.agent_pubkey(),
        conductors[2].dump_network_stats().await.unwrap().peer_urls[0]
    );

    await_consistency(10, [&alice, &bob, &carol]).await.unwrap();

    // Shutdown Carol's conductor.
    conductors[2].shutdown().await;

    // Alice creates an invalid action.
    let _: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            AppString("entry1".into()),
        )
        .await;

    await_consistency(10, [&alice, &bob]).await.unwrap();

    // Bob should have issued a warrant against Alice.

    // Shutdown Alice and startup Carol.
    conductors[0].shutdown().await;
    conductors[2].startup(false).await;

    // Carol should receive the warrant against Alice.
    // The warrant and the warrant op should have been written to the authored databases.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let alice_pubkey = alice.agent_pubkey().clone();
            let warrants = conductors[2]
                .get_spaces()
                .dht_db(dna_hash)
                .unwrap()
                .test_read(move |txn| {
                    let store = CascadeTxnWrapper::from(txn);
                    // TODO: check_valid here should be removed once warrants are validated.
                    store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                });
            if warrants.len() == 1 {
                assert_eq!(warrants[0].warrant().warrantee, *alice.agent_pubkey());
                // Make sure that Bob authored the warrant and it's not been authored by Carol.
                assert_eq!(warrants[0].warrant().author, *bob.agent_pubkey());
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
}
