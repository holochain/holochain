#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::single_match)]

use hdk::prelude::*;
use holo_hash::ActionHash;
use holochain::sweettest::SweetConductorBatch;
use holochain::sweettest::{
    await_consistency, DynSweetRendezvous, SweetAgents, SweetConductor, SweetConductorConfig,
    SweetDnaFile, SweetLocalRendezvous,
};
use holochain::test_utils::inline_zomes::{simple_create_read_zome, simple_crud_zome};
use holochain::{retry_until_timeout, sweettest::SweetInlineZomes};
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::NetworkConfig;
use holochain_zome_types::{
    record::{Record, RecordEntry},
    Entry,
};
use kitsune2_api::DhtArc;

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn bad_network_gossip() {
    holochain_trace::test_run();

    let vous = SweetLocalRendezvous::new_raw().await;

    vous.drop_sig().await;

    let v1: DynSweetRendezvous = vous.clone();
    let mut c1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true).no_dpki(),
        v1,
    )
    .await;

    let v2: DynSweetRendezvous = vous.clone();
    let mut c2 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true).no_dpki(),
        v2,
    )
    .await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let a1 = SweetAgents::one(c1.keystore()).await;
    let a2 = SweetAgents::one(c2.keystore()).await;

    let (app1,) = c1
        .setup_app_for_agent("app", a1.clone(), &[dna_file.clone()])
        .await
        .unwrap()
        .into_tuple();
    let (app2,) = c2
        .setup_app_for_agent("app", a2.clone(), &[dna_file])
        .await
        .unwrap()
        .into_tuple();

    let hash: ActionHash = c1.call(&app1.zome("simple"), "create", ()).await;

    // should not be possible to achieve consistency if the sbd server is down
    // note, the 3 seconds is small because we don't want the test to take
    // a long time, but also, this check isn't as important as the next one
    // that ensures after the server is back up we DO get consistency!
    assert!(await_consistency(3, [&app1, &app2]).await.is_err());

    vous.start_sig().await;

    await_consistency(60, [&app1, &app2]).await.unwrap();

    let record: Option<Record> = c2.call(&app2.zome("simple"), "read", hash).await;
    let record = record.expect("Record was None: bobbo couldn't `get` it");

    assert_eq!(record.action().author(), &a1);
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );
}

/// Test that conductors with arcs clamped to zero do not gossip.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn get_with_zero_arc_2_way() {
    holochain_trace::test_run();

    // Standard config with arc clamped to zero and publishing off
    let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    conductors[0]
        .holochain_p2p()
        .test_set_full_arcs(dna_file.dna_hash().to_k2_space())
        .await;
    retry_until_timeout!(5_000, 500, {
        let alice_in_own_peer_store = conductors[0]
            .holochain_p2p()
            .peer_store(alice.dna_hash().clone())
            .await
            .unwrap()
            .get(alice.agent_pubkey().to_k2_agent())
            .await
            .unwrap()
            .unwrap();
        if alice_in_own_peer_store.storage_arc == DhtArc::FULL {
            break;
        }
    });
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
async fn gossip_happens_with_new_conductors() {
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

// /// Test that:
// /// - 6MB of data passes from node A to B,
// /// - then A shuts down and C starts up,
// /// - and then that same data passes from B to C.
// #[cfg(feature = "slow_tests")]
// #[tokio::test(flavor = "multi_thread")]
// #[cfg_attr(target_os = "macos", ignore = "flaky")]
// #[cfg_attr(target_os = "windows", ignore = "flaky")]
// async fn three_way_gossip() {
//     holochain_trace::test_run();
//     let config = ConductorConfig {
//         network: NetworkConfig {
//             disable_publish: true,
//             ..Default::default()
//         }
//         .with_gossip_initiate_interval_ms(1000)
//         .with_gossip_min_initiate_interval_ms(1000),
//         ..Default::default()
//     };
//     let mut conductors = SweetConductorBatch::from_config(2, config.clone()).await;
//     let start = Instant::now();

//     let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

//     let cells = conductors
//         .setup_app("app", [&dna_file])
//         .await
//         .unwrap()
//         .cells_flattened();

//     println!(
//         "Initial agents: {:#?}",
//         cells
//             .iter()
//             .map(|c| c.agent_pubkey().to_k2_agent())
//             .collect::<Vec<_>>()
//     );

//     let zomes: Vec<_> = cells
//         .iter()
//         .map(|c| c.zome(SweetInlineZomes::COORDINATOR))
//         .collect();

//     let size = 3;
//     let num = 2;

//     let mut hashes = vec![];
//     for i in 0..num {
//         let bytes = vec![42u8 + i as u8; size];
//         let hash: ActionHash = conductors[0].call(&zomes[0], "create_bytes", bytes).await;
//         hashes.push(hash);
//     }

//     await_consistency(20, [&cells[0], &cells[1]]).await.unwrap();

//     println!(
//         "Done waiting for consistency between first two nodes. Elapsed: {:?}",
//         start.elapsed()
//     );

//     let records_0: Vec<Option<Record>> = conductors[0]
//         .call(&zomes[0], "read_multi", hashes.clone())
//         .await;
//     let records_1: Vec<Option<Record>> = conductors[1]
//         .call(&zomes[1], "read_multi", hashes.clone())
//         .await;
//     assert_eq!(
//         records_1.iter().filter(|r| r.is_some()).count(),
//         num,
//         "couldn't get records at positions: {:?}",
//         records_1
//             .iter()
//             .enumerate()
//             .filter_map(|(i, r)| r.is_none().then_some(i))
//             .collect::<Vec<_>>()
//     );
//     assert_eq!(records_0, records_1);

//     // Forget the first node's peer info before it gets gossiped to the third node.
//     // NOTE: this simulates "leave network", which we haven't yet implemented. The test will work without this,
//     // but there is a high chance of a 60 second timeout which flakily slows down this test beyond any acceptable duration.
//     conductors[0].shutdown().await;

//     // Bring a third conductor online
//     conductors.add_conductor_from_config(config).await;

//     // conductors.persist_dbs();

//     let (cell,) = conductors[2]
//         .setup_app("app", [&dna_file])
//         .await
//         .unwrap()
//         .into_tuple();
//     let zome = cell.zome(SweetInlineZomes::COORDINATOR);
//     // SweetConductor::exchange_peer_info([&conductors[1], &conductors[2]]).await;

//     println!(
//         "Newcomer agent joined: agent={:#?}",
//         cell.agent_pubkey().to_k2_agent()
//     );

//     conductors[2]
//         .require_initial_gossip_activity_for_cell(&cell, 2, Duration::from_secs(60))
//         .await
//         .unwrap();

//     println!(
//         "Initial gossip activity completed. Elapsed: {:?}",
//         start.elapsed()
//     );

//     await_consistency(60, [&cells[1], &cell]).await.unwrap();

//     println!(
//         "Done waiting for consistency between last two nodes. Elapsed: {:?}",
//         start.elapsed()
//     );

//     let records_2: Vec<Option<Record>> = conductors[2].call(&zome, "read_multi", hashes).await;
//     assert_eq!(
//         records_2.iter().filter(|r| r.is_some()).count(),
//         num,
//         "couldn't get records at positions: {:?}",
//         records_2
//             .iter()
//             .enumerate()
//             .filter_map(|(i, r)| r.is_none().then_some(i))
//             .collect::<Vec<_>>()
//     );
//     assert_eq!(records_2, records_1);
// }
