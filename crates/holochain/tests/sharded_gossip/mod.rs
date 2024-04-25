use std::time::{Duration, Instant};

use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::test_utils::inline_zomes::{
    batch_create_zome, simple_create_read_zome, simple_crud_zome,
};
use holochain::test_utils::network_simulation::{data_zome, generate_test_data};
use holochain::test_utils::WaitFor;
use holochain::{
    conductor::ConductorBuilder, test_utils::consistency::local_machine_session_with_hashes,
};
use holochain_p2p::*;
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use kitsune_p2p_types::config::RECENT_THRESHOLD_DEFAULT;

fn make_tuning(
    publish: bool,
    recent: bool,
    historical: bool,
    recent_threshold: Option<u64>,
) -> KitsuneP2pTuningParams {
    let mut tuning = KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.disable_publish = !publish;
    tuning.disable_recent_gossip = !recent;
    tuning.disable_historical_gossip = !historical;
    tuning.danger_gossip_recent_threshold_secs =
        recent_threshold.unwrap_or(RECENT_THRESHOLD_DEFAULT.as_secs());

    tuning.gossip_inbound_target_mbps = 10000.0;
    tuning.gossip_outbound_target_mbps = 10000.0;
    tuning.gossip_historic_outbound_target_mbps = 10000.0;
    tuning.gossip_historic_inbound_target_mbps = 10000.0;

    // This allows attempting to contact an offline node to timeout quickly,
    // so we can fallback to the next one
    tuning.default_rpc_single_timeout_ms = 3_000;
    tuning.gossip_round_timeout_ms = 10_000;
    tuning.bootstrap_check_delay_backoff_multiplier = 1;

    tuning
}

#[derive(Clone, Debug)]
struct TestConfig {
    publish: bool,
    recent: bool,
    historical: bool,
    bootstrap: bool,
    recent_threshold: Option<u64>,
}

impl From<TestConfig> for SweetConductorConfig {
    fn from(tc: TestConfig) -> Self {
        let TestConfig {
            publish,
            recent,
            historical,
            bootstrap,
            recent_threshold,
        } = tc;
        let tuning = make_tuning(publish, recent, historical, recent_threshold);
        SweetConductorConfig::rendezvous(bootstrap).set_tuning_params(tuning)
    }
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn fullsync_sharded_gossip_low_data() -> anyhow::Result<()> {
    let _g = holochain_trace::test_run();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        NUM_CONDUCTORS,
        TestConfig {
            publish: false,
            recent: true,
            historical: true,
            bootstrap: true,
            recent_threshold: None,
        },
    )
    .await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    conductors[0]
        .require_initial_gossip_activity_for_cell(
            &alice,
            NUM_CONDUCTORS as u32,
            Duration::from_secs(90),
        )
        .await;

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency(60, [&alice, &bobbo]).await.unwrap();
    // let p2p = conductors[0].envs().p2p().lock().values().next().cloned().unwrap();
    // holochain_state::prelude::dump_tmp(&p2p);
    // holochain_state::prelude::dump_tmp(&alice.env());
    // Verify that bobbo can run "read" on his cell and get alice's Action
    let record: Option<Record> = conductors[1]
        .call(&bobbo.zome("simple"), "read", hash)
        .await;
    let record = record.expect("Record was None: bobbo couldn't `get` it");

    // Assert that the Record bobbo sees matches what alice committed
    assert_eq!(record.action().author(), alice.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn fullsync_sharded_gossip_high_data() -> anyhow::Result<()> {
    holochain_trace::test_run();

    const NUM_CONDUCTORS: usize = 3;
    const NUM_OPS: usize = 100;

    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        NUM_CONDUCTORS,
        <TestConfig as Into<SweetConductorConfig>>::into(TestConfig {
            publish: false,
            recent: false,
            historical: true,
            bootstrap: true,
            recent_threshold: Some(0),
        })
        .tune_conductor(|p| {
            // Running too often here seems to not give other things enough time to process these ops. 2s seems to be a good middle ground
            // to make this test pass and be stable.
            p.sys_validation_retry_delay = Some(std::time::Duration::from_secs(2));
        }),
    )
    .await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("zome", batch_create_zome())).await;

    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    conductors[0]
        .require_initial_gossip_activity_for_cell(
            &alice,
            NUM_CONDUCTORS as u32,
            Duration::from_secs(90),
        )
        .await;

    // Call the "create" zome fn on Alice's app
    let hashes: Vec<ActionHash> = conductors[0]
        .call(&alice.zome("zome"), "create_batch", NUM_OPS)
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency(20, [&alice, &bobbo, &carol])
        .await
        .unwrap();

    let mut all_op_hashes = vec![];

    for i in 0..NUM_CONDUCTORS {
        let mut hashes: Vec<_> = conductors[i]
            .get_spaces()
            .handle_query_op_hashes(
                dna_file.dna_hash(),
                holochain_p2p::dht_arc::DhtArcSet::Full,
                kitsune_p2p::event::full_time_window(),
                100000000,
                true,
            )
            .await
            .unwrap()
            .unwrap()
            .0;

        hashes.sort();
        all_op_hashes.push(hashes);
    }

    assert_eq!(all_op_hashes[0].len(), all_op_hashes[1].len());
    assert_eq!(all_op_hashes[0], all_op_hashes[1]);
    assert_eq!(all_op_hashes[1].len(), all_op_hashes[2].len());
    assert_eq!(all_op_hashes[1], all_op_hashes[2]);

    // Verify that bobbo can run "read" on his cell and get alice's Action
    let element: Option<Record> = conductors[1]
        .call(&bobbo.zome("zome"), "read", hashes[0].clone())
        .await;
    let element = element.expect("Record was None: bobbo couldn't `get` it");

    // Assert that the Record bobbo sees matches what alice committed
    assert_eq!(element.action().author(), alice.agent_pubkey());
    assert!(matches!(
        *element.entry(),
        RecordEntry::Present(Entry::App(_))
    ));

    Ok(())
}

/// Test that conductors with arcs clamped to zero do not gossip.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn test_zero_arc_get_links() {
    holochain_trace::test_run();

    // Standard config with arc clamped to zero
    let mut tuning = make_tuning(true, true, true, None);
    tuning.gossip_arc_clamping = "empty".into();
    let config = SweetConductorConfig::standard().set_tuning_params(tuning);

    let mut conductor0 = SweetConductor::from_config(config).await;
    let mut conductor1 = SweetConductor::from_standard_config().await;

    let tw = holochain_wasm_test_utils::TestWasm::Link;
    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![tw]).await;
    let app0 = conductor0.setup_app("app", [&dna_file]).await.unwrap();
    let _ = conductor1.setup_app("app", [&dna_file]).await.unwrap();
    let (cell0,) = app0.into_tuple();

    // conductors.exchange_peer_info().await;

    let zome0 = cell0.zome(tw);
    let _hash0: ActionHash = conductor0.call(&zome0, "create_link", ()).await;

    let links: Vec<Link> = conductor0.call(&zome0, "get_links", ()).await;
    assert_eq!(links.len(), 1);
}

/// Test that conductors with arcs clamped to zero do not gossip.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn test_zero_arc_no_gossip_2way() {
    holochain_trace::test_run();

    // Standard config

    let config_0 = TestConfig {
        publish: true,
        recent: true,
        historical: true,
        bootstrap: true,
        recent_threshold: None,
    }
    .into();

    // Standard config with arc clamped to zero and publishing off
    // This should result in no publishing or gossip
    let mut tuning_1 = make_tuning(false, true, true, None);
    tuning_1.gossip_arc_clamping = "empty".into();
    let config_1 = SweetConductorConfig::rendezvous(true).set_tuning_params(tuning_1);

    let mut conductors = SweetConductorBatch::from_configs_rendezvous([config_0, config_1]).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let ((cell_0,), (cell_1,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;

    let zome_0 = cell_0.zome(SweetInlineZomes::COORDINATOR);
    let hash_0: ActionHash = conductors[0]
        .call(&zome_0, "create_string", "hi".to_string())
        .await;

    let zome_1 = cell_1.zome(SweetInlineZomes::COORDINATOR);
    let hash_1: ActionHash = conductors[1]
        .call(&zome_1, "create_string", "hi".to_string())
        .await;

    // can't await consistency because one node is neither publishing nor gossiping, and is relying only on `get`

    let record_01: Option<Record> = conductors[0].call(&zome_0, "read", hash_1.clone()).await;
    let record_10: Option<Record> = conductors[1].call(&zome_1, "read", hash_0.clone()).await;

    // 1 is not a valid target for the get, and 0 did not publish, so 0 can't get 1's data.
    assert!(record_01.is_none());

    // 1 can get 0's data, though.
    assert!(record_10.is_some());
}

/// Test that conductors with arcs clamped to zero do not gossip.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn test_zero_arc_no_gossip_4way() {
    use futures::future::join_all;

    holochain_trace::test_run();

    let configs = [
        // Standard config
        <TestConfig as Into<SweetConductorConfig>>::into(TestConfig {
            publish: true,
            recent: true,
            historical: true,
            bootstrap: true,
            recent_threshold: None,
        })
        .tune_conductor(|params| {
            // Speed up sys validation retry when gets hit a conductor that isn't yet serving the requested data
            params.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
        }),
        // Publishing turned off
        <TestConfig as Into<SweetConductorConfig>>::into(TestConfig {
            publish: false,
            recent: true,
            historical: true,
            bootstrap: true,
            recent_threshold: None,
        })
        .tune_conductor(|params| {
            // Speed up sys validation retry when gets hit a conductor that isn't yet serving the requested data
            params.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
        }),
        {
            // Standard config with arc clamped to zero
            let mut tuning = make_tuning(true, true, true, None);
            tuning.gossip_arc_clamping = "empty".into();
            SweetConductorConfig::rendezvous(true)
                .tune_conductor(|params| {
                    // Speed up sys validation retry when gets hit a conductor that isn't yet serving the requested data
                    params.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
                })
                .set_tuning_params(tuning)
        },
        {
            // Publishing turned off, arc clamped to zero
            let mut tuning = make_tuning(false, true, true, None);
            tuning.gossip_arc_clamping = "empty".into();
            SweetConductorConfig::rendezvous(true)
                .tune_conductor(|params| {
                    // Speed up sys validation retry when gets hit a conductor that isn't yet serving the requested data
                    params.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
                })
                .set_tuning_params(tuning)
        },
    ];

    let mut conductors = SweetConductorBatch::from_configs_rendezvous(configs).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let dna_hash = dna_file.dna_hash().clone();

    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();
    let cells = apps.cells_flattened();
    let zomes: Vec<_> = cells
        .iter()
        .map(|c| c.zome(SweetInlineZomes::COORDINATOR))
        .collect();

    // Ensure that each node has one agent in its peer store, for the single app installed.
    for (i, cell) in cells.iter().enumerate() {
        let stored_agents = holochain::conductor::p2p_agent_store::all_agent_infos(
            conductors[i]
                .get_spaces()
                .p2p_agents_db(&dna_hash)
                .unwrap()
                .into(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|i| AgentPubKey::from_kitsune(&i.agent()))
        .collect::<Vec<_>>();
        assert_eq!(stored_agents, vec![cell.agent_pubkey().clone()]);
    }

    conductors.exchange_peer_info().await;

    // Ensure that each node has all agents in their local p2p store.
    for c in conductors.iter() {
        let stored_agents = holochain::conductor::p2p_agent_store::all_agent_infos(
            c.get_spaces().p2p_agents_db(&dna_hash).unwrap().into(),
        )
        .await
        .unwrap()
        .len();
        assert_eq!(stored_agents, conductors.len());
    }

    // Have each conductor create an entry
    let hashes: Vec<ActionHash> = join_all(
        conductors
            .iter()
            .enumerate()
            .map(|(i, c)| c.call(&zomes[i], "create_string", format!("{}", i))),
    )
    .await;

    // Have each conductor attempt to get every other conductor's entry,
    // retrying for a certain amount of time until the entry could be successfully retrieved,
    // then testing for success.
    //
    // Nobody should be able to get conductor 3's entry, because it is not publishing
    // and not gossiping due to zero arc.
    let _: Vec<()> = join_all(conductors.iter().enumerate().flat_map(|(i, c)| {
        hashes
            .iter()
            .enumerate()
            .map(|(j, hash)| {
                let zome = zomes[i].clone();
                async move {
                    let assertion = |x: bool| {
                        if j == 3 && i != j {
                            assert!(!x, "Node 3's data should not be accessible by anyone but itself. i={}, j={}", i, j);
                        } else {
                            assert!(x, "All nodes should be able to get all data except for node 3's. i={}, j={}", i, j);
                        }
                    };
                    holochain::wait_for!(
                        WaitFor::new(std::time::Duration::from_secs(5), 10),
                        c.call::<_, Option<Record>>(&zome, "read", hash.clone())
                            .await
                            .is_some(),
                        |x: &bool| {
                            if j == 3 && i != j {
                                !x
                            } else {
                                *x
                            }
                        },
                        assertion
                    );
                }
            })
            .collect::<Vec<_>>()
    }))
    .await;
}

/// Test that when the conductor shuts down, gossip does not continue,
/// and when it restarts, gossip resumes.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "deal with connections closing and banning for 10s"]
async fn test_gossip_shutdown() {
    holochain_trace::test_run();
    let mut conductors = SweetConductorBatch::from_config_rendezvous(
        2,
        TestConfig {
            publish: false,
            recent: true,
            historical: true,
            bootstrap: true,
            recent_threshold: None,
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

    // After shutting down conductor 0, test that gossip doesn't happen within 3 seconds
    // of peer discovery (assuming it will never happen)
    conductors[0].shutdown().await;
    conductors.exchange_peer_info().await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert!(record.is_none());

    // Ensure that gossip loops resume upon startup
    conductors[0].startup().await;

    await_consistency(60, [&cell_0, &cell_1]).await.unwrap();
    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

/// Test that when a new conductor joins, gossip picks up existing data without needing a publish.
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "This test is potentially useful but uses sleeps and has never failed.
            Run it again in the future to see if it fails, and if so, rewrite it without sleeps."]
async fn test_gossip_startup() {
    holochain_trace::test_run();
    let config = || {
        SweetConductorConfig::standard().tune(|t| {
            t.danger_gossip_recent_threshold_secs = 1;
            t.default_rpc_single_timeout_ms = 3_000;
        })
    };

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mk_conductor = || async {
        let cfg = config();
        assert!(cfg.network.is_tx5());
        let mut conductor =
            SweetConductor::from_config_rendezvous(cfg, SweetLocalRendezvous::new().await).await;
        // let mut conductor = SweetConductor::from_config(config()).await;
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
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let (conductor1, cell1, zome1) = mk_conductor().await;

    // Wait a bit so that conductor 0 doesn't publish in the next step.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    SweetConductor::exchange_peer_info([&conductor0, &conductor1]).await;

    await_consistency(60, [&cell0, &cell1]).await.unwrap();
    let record: Option<Record> = conductor1.call(&zome1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn three_way_gossip_recent() {
    hc_sleuth::init_subscriber();
    let config = TestConfig {
        publish: false,
        recent: true,
        historical: false,
        // NOTE: disable bootstrap so we can selectively ignore the shut-down conductor
        bootstrap: false,
        recent_threshold: None,
    };
    three_way_gossip(config.into()).await;
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn three_way_gossip_historical() {
    hc_sleuth::init_subscriber();
    let config = TestConfig {
        publish: false,
        recent: false,
        historical: true,
        // NOTE: disable bootstrap so we can selectively ignore the shut-down conductor
        bootstrap: false,
        recent_threshold: Some(0),
    };
    three_way_gossip(config.into()).await;
}

/// Test that:
/// - 6MB of data passes from node A to B,
/// - then A shuts down and C starts up,
/// - and then that same data passes from B to C.
async fn three_way_gossip(config: holochain::sweettest::SweetConductorConfig) {
    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config.clone()).await;
    let start = Instant::now();

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let cells: Vec<_> = futures::future::join_all(conductors.iter_mut().map(|c| async {
        let (cell,) = c.setup_app("app", [&dna_file]).await.unwrap().into_tuple();
        cell
    }))
    .await;

    conductors.exchange_peer_info().await;

    println!(
        "Initial agents: {:#?}",
        cells
            .iter()
            .map(|c| c.agent_pubkey().to_kitsune())
            .collect::<Vec<_>>()
    );

    let zomes: Vec<_> = cells
        .iter()
        .map(|c| c.zome(SweetInlineZomes::COORDINATOR))
        .collect();

    let size = 3_000_000;
    let num = 2;

    let mut hashes = vec![];
    for i in 0..num {
        let bytes = vec![42u8 + i as u8; size];
        let hash: ActionHash = conductors[0].call(&zomes[0], "create_bytes", bytes).await;
        hashes.push(hash);
    }

    await_consistency(10, [&cells[0], &cells[1]]).await.unwrap();

    println!(
        "Done waiting for consistency between first two nodes. Elapsed: {:?}",
        start.elapsed()
    );

    let records_0: Vec<Option<Record>> = conductors[0]
        .call(&zomes[0], "read_multi", hashes.clone())
        .await;
    let records_1: Vec<Option<Record>> = conductors[1]
        .call(&zomes[1], "read_multi", hashes.clone())
        .await;
    assert_eq!(
        records_1.iter().filter(|r| r.is_some()).count(),
        num,
        "couldn't get records at positions: {:?}",
        records_1
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.is_none().then_some(i))
            .collect::<Vec<_>>()
    );
    assert_eq!(records_0, records_1);

    // Forget the first node's peer info before it gets gossiped to the third node.
    // NOTE: this simulates "leave network", which we haven't yet implemented. The test will work without this,
    // but there is a high chance of a 60 second timeout which flakily slows down this test beyond any acceptable duration.
    conductors.forget_peer_info([cells[0].agent_pubkey()]).await;
    conductors[0].shutdown().await;

    // Bring a third conductor online
    conductors.add_conductor_from_config(config).await;

    conductors.persist_dbs();

    let (cell,) = conductors[2]
        .setup_app("app", [&dna_file])
        .await
        .unwrap()
        .into_tuple();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);
    SweetConductor::exchange_peer_info([&conductors[1], &conductors[2]]).await;

    println!(
        "Newcomer agent joined: scope={}, agent={:#?}",
        conductors[2].get_config().sleuth_id(),
        cell.agent_pubkey().to_kitsune()
    );

    conductors[2]
        .require_initial_gossip_activity_for_cell(&cell, 2, Duration::from_secs(30))
        .await;

    println!(
        "Initial gossip activity completed. Elapsed: {:?}",
        start.elapsed()
    );

    await_consistency_advanced(10, [(&cells[0], false), (&cells[1], true), (&cell, true)])
        .await
        .unwrap();

    println!(
        "Done waiting for consistency between last two nodes. Elapsed: {:?}",
        start.elapsed()
    );

    let records_2: Vec<Option<Record>> = conductors[2].call(&zome, "read_multi", hashes).await;
    assert_eq!(
        records_2.iter().filter(|r| r.is_some()).count(),
        num,
        "couldn't get records at positions: {:?}",
        records_2
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.is_none().then_some(i))
            .collect::<Vec<_>>()
    );
    assert_eq!(records_2, records_1);
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_local_gossip() -> anyhow::Result<()> {
    use holochain::{sweettest::SweetConductor, test_utils::inline_zomes::simple_create_read_zome};

    let _g = holochain_trace::test_run();

    let mut conductor = SweetConductor::from_config_rendezvous(
        TestConfig {
            publish: false,
            recent: true,
            historical: true,
            bootstrap: true,
            recent_threshold: None,
        },
        SweetLocalRendezvous::new().await,
    )
    .await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let alice = conductor.setup_app("app", [&dna_file]).await.unwrap();

    let (alice,) = alice.into_tuple();
    let bobbo = conductor.setup_app("app2 ", [&dna_file]).await.unwrap();

    let (bobbo,) = bobbo.into_tuple();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor.call(&alice.zome("simple"), "create", ()).await;

    // Wait long enough for Bob to receive gossip
    await_consistency(10, [&alice, &bobbo]).await.unwrap();

    // Verify that bobbo can run "read" on his cell and get alice's Action
    let record: Option<Record> = conductor.call(&bobbo.zome("simple"), "read", hash).await;
    let record = record.expect("Record was None: bobbo couldn't `get` it");

    // Assert that the Record bobbo sees matches what alice committed
    assert_eq!(record.action().author(), alice.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

/*

TODO: rewrite for tx5, or simply learn from it and remove it / rewrite it.
      either way, this is quite old and has not been used since 2022.




#[cfg(feature = "test_utils")]
async fn run_bootstrap(
    peer_data: impl Iterator<Item = kitsune_p2p::agent_store::AgentInfoSigned>,
) -> (url2::Url2, kitsune_p2p_bootstrap::BootstrapShutdown) {
    let mut url = url2::url2!("http://127.0.0.1:0");
    let (driver, addr, shutdown) = kitsune_p2p_bootstrap::run(([127, 0, 0, 1], 0), vec![])
        .await
        .unwrap();
    tokio::spawn(driver);
    let client = reqwest::Client::new();
    url.set_port(Some(addr.port())).unwrap();
    for info in peer_data {
        let _: Option<()> = do_api(url.clone(), "put", info, &client).await.unwrap();
    }
    (url, shutdown)
}

async fn do_api<I: serde::Serialize, O: serde::de::DeserializeOwned>(
    url: kitsune_p2p::dependencies::url2::Url2,
    op: &str,
    input: I,
    client: &reqwest::Client,
) -> Option<O> {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, &input).unwrap();
    let res = client
        .post(url.as_str())
        .body(body_data)
        .header("X-Op", op)
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await
        .unwrap();

    Some(kitsune_p2p_types::codec::rmp_decode(&mut res.bytes().await.unwrap().as_ref()).unwrap())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Prototype test that is not suitable for CI"]
/// This is a prototype test to demonstrate how to create a
/// simulated network.
/// It tests one real agent talking to a number of simulated agents.
/// The simulated agents respond to initiated gossip rounds but do
/// not initiate there own.
/// The simulated agents respond to gets correctly.
/// The test checks that the real agent:
/// - Can reach consistency.
/// - Initiates with all the simulated agents that it should.
/// - Does not route gets or publishes to the wrong agents.
async fn mock_network_sharded_gossip() {
    use std::sync::atomic::AtomicUsize;

    use hdk::prelude::*;
    use holochain_p2p::dht_arc::DhtLocation;
    use holochain_p2p::mock_network::{GossipProtocol, MockScenario};
    use holochain_p2p::{
        dht_arc::DhtArcSet,
        mock_network::{
            AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg,
        },
    };
    use kitsune_p2p::gossip::sharded_gossip::test_utils::*;
    use kitsune_p2p_types::config::TransportConfig;
    use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;

    // Get the env var settings for number of simulated agents and
    // the minimum number of ops that should be held by each agent
    // if there are some or use defaults.
    let (num_agents, min_ops) = std::env::var_os("NUM_AGENTS")
        .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
        .and_then(|na| {
            std::env::var_os("MIN_OPS")
                .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
                .map(|mo| (na, mo))
        })
        .unwrap_or((100, 10));

    // Check if we should for new data to be generated even if it already exists.
    let force_new_data = std::env::var_os("FORCE_NEW_DATA").is_some();

    let _g = holochain_trace::test_run();

    // Generate or use cached test data.
    let (data, mut conn) = generate_test_data(num_agents, min_ops, false, force_new_data).await;
    let data = Arc::new(data);

    // We have to use the same dna that was used to generate the test data.
    // This is a short coming I hope to overcome in future versions.
    let dna_file = data_zome(data.integrity_uuid.clone(), data.coordinator_uuid.clone()).await;

    // We are pretending that all simulated agents have all other agents (except the real agent)
    // for this test.

    // Create the one agent bloom.
    let agent_bloom = create_agent_bloom(data.agent_info(), None);

    // Create the simulated network.
    let (from_kitsune_tx, to_kitsune_rx, mut channel) = HolochainP2pMockChannel::channel(
        // Pass in the generated simulated peer data.
        data.agent_info().cloned().collect(),
        // We want to buffer up to 1000 network messages.
        1000,
        MockScenario {
            // Don't drop any individual messages.
            percent_drop_msg: 0.0,
            // A random 10% of simulated agents will never be online.
            percent_offline: 0.0,
            // Simulated agents will receive messages from within 50 to 100 ms.
            inbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
            // Simulated agents will send messages from within 50 to 100 ms.
            outbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
        },
    );

    // Share alice's peer data as it changes.
    let alice_info = Arc::new(parking_lot::Mutex::new(None));
    // Share the number of hashes alice should be holding.
    let num_hashes_alice_should_hold = Arc::new(AtomicUsize::new(0));

    // Create some oneshot to notify of any publishes to the wrong node.
    // TODO (david.b) - publish has been replaced by a combined receive_ops
    //let (bad_publish_tx, mut bad_publish_rx) = tokio::sync::oneshot::channel();

    // Create some oneshot to notify of any gets to the wrong node.
    let (bad_get_tx, mut bad_get_rx) = tokio::sync::oneshot::channel();
    // Track the agents that have been gossiped with.
    let (agents_gossiped_with_tx, mut agents_gossiped_with_rx) =
        tokio::sync::watch::channel(HashSet::new());

    // Spawn the task that will simulate the network.
    tokio::task::spawn({
        let data = data.clone();
        let alice_info = alice_info.clone();
        let num_hashes_alice_should_hold = num_hashes_alice_should_hold.clone();
        async move {
            let mut gossiped_ops = HashSet::new();
            let start_time = std::time::Instant::now();
            let mut agents_gossiped_with = HashSet::new();
            let mut num_missed_gossips = 0;
            let mut last_intervals = None;
            // TODO (david.b) - publish has been replaced by a
            //                  combined receive_ops
            //let mut bad_publish = Some(bad_publish_tx);
            let mut bad_get = Some(bad_get_tx);

            // Get the next network message.
            while let Some((msg, respond)) = channel.next().await {
                let alice: Option<AgentInfoSigned> = alice_info.lock().clone();
                let num_hashes_alice_should_hold =
                    num_hashes_alice_should_hold.load(std::sync::atomic::Ordering::Relaxed);

                let AddressedHolochainP2pMockMsg { agent, msg } = msg;
                let agent = Arc::new(agent);

                // Match on the message and create a response (if a response is needed).
                match msg {
                    HolochainP2pMockMsg::Wire { msg, .. } => match msg {
                        holochain_p2p::WireMessage::CallRemoteMulti { .. } => {
                            debug!("CallRemoteMulti")
                        }
                        holochain_p2p::WireMessage::CallRemote { .. } => debug!("CallRemote"),
                        holochain_p2p::WireMessage::PublishCountersign { .. } => {
                            debug!("PublishCountersign")
                        }
                        /* (david.b) TODO - this has been replaced by
                         *                  combined `receive_ops`
                        holochain_p2p::WireMessage::Publish { ops, .. } => {
                            if bad_publish.is_some() {
                                let arc = data.agent_to_arc[&agent];
                                if ops
                                    .into_iter()
                                    .any(|op| !arc.contains(op.dht_basis().get_loc()))
                                {
                                    bad_publish.take().unwrap().send(()).unwrap();
                                }
                            }
                        }
                        */
                        holochain_p2p::WireMessage::ValidationReceipts { receipts: _ } => {
                            debug!("Validation Receipt")
                        }
                        holochain_p2p::WireMessage::Get { dht_hash, options } => {
                            let txn = conn
                                .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
                                .unwrap();
                            let ops = holochain_cascade::test_utils::handle_get_txn(
                                &txn,
                                dht_hash.clone(),
                                options,
                            );
                            if bad_get.is_some() {
                                let arq = data.agent_to_arq[&agent];

                                if !arq.to_dht_arc_range_std().contains(dht_hash.get_loc()) {
                                    bad_get.take().unwrap().send(()).unwrap();
                                }
                            }
                            let ops: Vec<u8> =
                                UnsafeBytes::from(SerializedBytes::try_from(ops).unwrap()).into();
                            let msg = HolochainP2pMockMsg::CallResp(ops.into());
                            respond.unwrap().respond(msg);
                        }
                        holochain_p2p::WireMessage::GetMeta { .. } => debug!("get_meta"),
                        holochain_p2p::WireMessage::GetLinks { .. } => debug!("get_links"),
                        holochain_p2p::WireMessage::CountLinks { .. } => debug!("count_links"),
                        holochain_p2p::WireMessage::GetAgentActivity { .. } => {
                            debug!("get_agent_activity")
                        }
                        holochain_p2p::WireMessage::MustGetAgentActivity { .. } => {
                            debug!("must_get_agent_activity")
                        }
                        holochain_p2p::WireMessage::CountersigningSessionNegotiation { .. } => {
                            debug!("countersigning_session_negotiation")
                        }
                    },
                    HolochainP2pMockMsg::CallResp(_) => debug!("CallResp"),
                    HolochainP2pMockMsg::PeerGet(_) => debug!("PeerGet"),
                    HolochainP2pMockMsg::PeerGetResp(_) => debug!("PeerGetResp"),
                    HolochainP2pMockMsg::PeerQuery(_) => debug!("PeerQuery"),
                    HolochainP2pMockMsg::PeerQueryResp(_) => debug!("PeerQueryResp"),
                    HolochainP2pMockMsg::PeerUnsolicited(_) => debug!("PeerUnsolicited"),
                    HolochainP2pMockMsg::MetricExchange(_) => debug!("MetricExchange"),
                    HolochainP2pMockMsg::Gossip {
                        dna,
                        module,
                        gossip,
                    } => {
                        if let kitsune_p2p::GossipModuleType::ShardedRecent = module {
                            #[allow(irrefutable_let_patterns)]
                            if let GossipProtocol::Sharded(gossip) = gossip {
                                use kitsune_p2p::gossip::sharded_gossip::*;
                                match gossip {
                                    ShardedGossipWire::Initiate(Initiate { intervals, .. }) => {
                                        // Capture the intervals from alice.
                                        // This works because alice will only initiate with one simulated
                                        // agent at a time.
                                        last_intervals = Some(intervals);
                                        let arc = data.agent_to_arq[&agent];
                                        let agent_info = data.agent_to_info[&agent].clone();
                                        let interval = arc;

                                        // If we have info for alice check the overlap.
                                        if let Some(alice) = &alice {
                                            let a = alice.storage_arc();
                                            let b = interval;
                                            debug!("{}\n{}", a.to_ascii(10), b.to_ascii_std(10));
                                            let a: DhtArcSet = a.inner().into();
                                            let b: DhtArcSet = b.to_dht_arc_range_std().into();
                                            if !a.overlap(&b) {
                                                num_missed_gossips += 1;
                                            }
                                        }

                                        // Record that this simulated agent was initiated with.
                                        agents_gossiped_with.insert(agent.clone());
                                        agents_gossiped_with_tx
                                            .send(agents_gossiped_with.clone())
                                            .unwrap();

                                        // Accept the initiate.
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(
                                                    vec![interval.to_bounds_std()],
                                                    vec![agent_info],
                                                ),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

                                        // Create an ops bloom and send it back.
                                        let window = (Timestamp::now()
                                            - std::time::Duration::from_secs(60 * 60))
                                        .unwrap()
                                            ..Timestamp::now();
                                        let this_agent_hashes: Vec<_> = data
                                            .hashes_authority_for(&agent)
                                            .into_iter()
                                            .filter(|h| {
                                                window.contains(&data.ops[h].action().timestamp())
                                            })
                                            .map(|k| data.op_hash_to_kit[&k].clone())
                                            .collect();
                                        let filter = if this_agent_hashes.is_empty() {
                                            EncodedTimedBloomFilter::MissingAllHashes {
                                                time_window: window,
                                            }
                                        } else {
                                            let filter = create_op_bloom(this_agent_hashes);

                                            EncodedTimedBloomFilter::HaveHashes {
                                                time_window: window,
                                                filter,
                                            }
                                        };
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::op_bloom(filter, true),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

                                        // Create an agent bloom and send it.
                                        if let Some(ref agent_bloom) = agent_bloom {
                                            let msg = HolochainP2pMockMsg::Gossip {
                                                dna: dna.clone(),
                                                module: module,
                                                gossip: GossipProtocol::Sharded(
                                                    ShardedGossipWire::agents(agent_bloom.clone()),
                                                ),
                                            };
                                            channel.send(msg.addressed((*agent).clone())).await;
                                        }
                                    }
                                    ShardedGossipWire::OpBloom(OpBloom {
                                        missing_hashes, ..
                                    }) => {
                                        // We have received an ops bloom so we can respond with any missing
                                        // hashes if there are nay.
                                        let this_agent_hashes = data.hashes_authority_for(&agent);
                                        let num_this_agent_hashes = this_agent_hashes.len();
                                        let hashes = this_agent_hashes.iter().map(|h| {
                                            (
                                                data.ops[h].action().timestamp(),
                                                &data.op_hash_to_kit[h],
                                            )
                                        });

                                        let missing_hashes =
                                            check_ops_bloom(hashes, missing_hashes);
                                        let missing_hashes = match &last_intervals {
                                            Some(intervals) => missing_hashes
                                                .into_iter()
                                                .filter(|hash| {
                                                    intervals[0].to_dht_arc_range_std().contains(
                                                        data.op_to_loc[&data.op_kit_to_hash[*hash]],
                                                    )
                                                })
                                                .cloned()
                                                .collect(),
                                            None => vec![],
                                        };
                                        gossiped_ops.extend(missing_hashes.iter().cloned());

                                        let num_gossiped = gossiped_ops.len();
                                        let p_done = num_gossiped as f64
                                            / num_hashes_alice_should_hold as f64
                                            * 100.0;
                                        let avg_gossip_freq = start_time
                                            .elapsed()
                                            .checked_div(agents_gossiped_with.len() as u32)
                                            .unwrap_or_default();
                                        let avg_gossip_size =
                                            num_gossiped / agents_gossiped_with.len();
                                        let time_to_completion = num_hashes_alice_should_hold
                                            .checked_sub(num_gossiped)
                                            .and_then(|n| n.checked_div(avg_gossip_size))
                                            .unwrap_or_default()
                                            as u32
                                            * avg_gossip_freq;
                                        let (overlap, max_could_get) = alice
                                            .as_ref()
                                            .map(|alice| {
                                                let arc = data.agent_to_arq[&agent];
                                                let a = alice.storage_arc();
                                                let b = arc.to_dht_arc_range_std();
                                                let num_should_hold = this_agent_hashes
                                                    .iter()
                                                    .filter(|hash| {
                                                        let loc = data.op_to_loc[*hash];
                                                        alice.storage_arc().contains(loc)
                                                    })
                                                    .count();
                                                (a.overlap_coverage(&b) * 100.0, num_should_hold)
                                            })
                                            .unwrap_or((0.0, 0));

                                        // Print out some stats.
                                        debug!(
                                            "Gossiped with {}, got {} of {} ops, overlap: {:.2}%, max could get {}, {:.2}% done, avg freq of gossip {:?}, est finish in {:?}",
                                            agent,
                                            missing_hashes.len(),
                                            num_this_agent_hashes,
                                            overlap,
                                            max_could_get,
                                            p_done,
                                            avg_gossip_freq,
                                            time_to_completion
                                        );
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna,
                                            module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::missing_op_hashes(
                                                    missing_hashes
                                                        .into_iter()
                                                        .map(|h| kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized::new(h, None))
                                                        .collect(),
                                                    MissingOpsStatus::AllComplete as u8,
                                                ),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::MissingOpHashes(MissingOpHashes {
                                        ops,
                                        ..
                                    }) => {
                                        debug!(
                                            "Gossiped with {} {} out of {}, who sent {} ops and gossiped with {} nodes outside of arc",
                                            agent,
                                            agents_gossiped_with.len(),
                                            data.num_agents(),
                                            ops.len(),
                                            num_missed_gossips
                                        );
                                    }
                                    ShardedGossipWire::OpRegions(_) => todo!("must implement"),

                                    ShardedGossipWire::Agents(_) => {}
                                    ShardedGossipWire::MissingAgents(_) => {}
                                    ShardedGossipWire::Accept(_) => (),
                                    ShardedGossipWire::NoAgents(_) => (),
                                    ShardedGossipWire::AlreadyInProgress(_) => (),
                                    ShardedGossipWire::Busy(_) => (),
                                    ShardedGossipWire::Error(_) => (),
                                    ShardedGossipWire::OpBatchReceived(_) => (),
                                }
                            }
                        }
                    }
                    HolochainP2pMockMsg::Failure(reason) => panic!("Failure: {}", reason),
                    HolochainP2pMockMsg::PublishedAgentInfo { .. } => todo!(),
                }
            }
        }
    });

    // Create the mock network.
    let mock_network =
        kitsune_p2p::test_util::mock_network::mock_network(from_kitsune_tx, to_kitsune_rx);
    let mock_network: AdapterFactory = Arc::new(mock_network);

    // Setup the network.
    let mut tuning = KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_dynamic_arcs = true;

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![TransportConfig::Mock {
        mock_network: mock_network.into(),
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::empty();
    config.network = network;

    // Add it to the conductor builder.
    let builder = ConductorBuilder::new().config(config);
    let mut conductor = SweetConductor::from_builder(builder).await;

    // Add in all the agent info.
    conductor
        .add_agent_infos(data.agent_to_info.values().cloned().collect())
        .await
        .unwrap();

    // Install the real agent alice.
    let apps = conductor.setup_app("app", [&dna_file]).await.unwrap();

    let (alice,) = apps.into_tuple();
    let alice_p2p_agents_db = conductor.get_p2p_db(alice.cell_id().dna_hash());
    let alice_kit = alice.agent_pubkey().to_kitsune();

    // Spawn a task to update alice's agent info.
    tokio::spawn({
        let alice_info = alice_info.clone();
        async move {
            loop {
                let info = alice_p2p_agents_db.p2p_get_agent(&alice_kit).await.unwrap();

                *alice_info.lock() = info;

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // Get the expected hashes and agents alice should gossip.
    let (
        // The expected hashes that should be held by alice.
        hashes_to_be_held,
        // The agents that alice should initiate with.
        agents_that_should_be_initiated_with,
    ): (
        Vec<(DhtLocation, Arc<DhtOpHash>)>,
        HashSet<Arc<AgentPubKey>>,
    ) = loop {
        if let Some(alice) = alice_info.lock().clone() {
            // if (alice.storage_arc().coverage() - data.coverage()).abs() < 0.01 {
            let hashes_to_be_held = data
                .ops
                .iter()
                .filter_map(|(hash, op)| {
                    let loc = op.dht_basis().get_loc();
                    alice
                        .storage_arc()
                        .contains(loc)
                        .then(|| (loc, hash.clone()))
                })
                .collect::<Vec<_>>();
            let agents_that_should_be_initiated_with = data
                .agents()
                .filter(|h| alice.storage_arc().contains(h.get_loc()))
                .cloned()
                .collect::<HashSet<_>>();
            num_hashes_alice_should_hold.store(
                hashes_to_be_held.len(),
                std::sync::atomic::Ordering::Relaxed,
            );
            debug!("Alice covers {} and the target coverage is {}. She should hold {} out of {} ops. She should gossip with {} agents", alice.storage_arc().coverage(), data.coverage(), hashes_to_be_held.len(), data.ops.len(), agents_that_should_be_initiated_with.len());
            break (hashes_to_be_held, agents_that_should_be_initiated_with);
            // }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    };

    // Wait for consistency to be reached.
    local_machine_session_with_hashes(
        vec![&conductor.raw_handle()],
        hashes_to_be_held.iter().map(|(l, h)| (*l, (**h).clone())),
        dna_file.dna_hash(),
        std::time::Duration::from_secs(60 * 60),
    )
    .await;

    // Check alice initiates with all the expected agents.
    // Note this won't pass if some agents are offline.
    while agents_gossiped_with_rx.changed().await.is_ok() {
        let new = agents_gossiped_with_rx.borrow();
        let diff: Vec<_> = agents_that_should_be_initiated_with
            .difference(&new)
            .collect();
        if diff.is_empty() {
            break;
        } else {
            debug!("Waiting for {} to initiated agents", diff.len());
        }
    }

    // TODO (david.b) - publish has been replaced by a combined receive_ops
    /*
    // Check if we got any publishes to the wrong agent.
    match bad_publish_rx.try_recv() {
        Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            panic!("Got a bad publish")
        }
        Err(_) => (),
    }
    */

    // Check if we got any gets to the wrong agent.
    match bad_get_rx.try_recv() {
        Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            panic!("Got a bad get")
        }
        Err(_) => (),
    }
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Prototype test that is not suitable for CI"]
/// This is a prototype test to demonstrate how to create a
/// simulated network.
/// It tests one real agent talking to a number of simulated agents.
/// The simulated agents respond to initiated gossip rounds but do
/// not initiate there own.
/// The simulated agents respond to gets correctly.
/// The test checks that the real agent:
/// - Can reach consistency.
/// - Initiates with all the simulated agents that it should.
/// - Does not route gets or publishes to the wrong agents.
async fn mock_network_sharding() {
    use std::sync::atomic::AtomicUsize;

    use hdk::prelude::*;
    use holochain_p2p::mock_network::{
        AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg,
    };
    use holochain_p2p::mock_network::{GossipProtocol, MockScenario};
    use holochain_p2p::AgentPubKeyExt;
    use holochain_state::prelude::*;
    use holochain_types::dht_op::WireOps;
    use holochain_types::record::WireRecordOps;
    use kitsune_p2p::gossip::sharded_gossip::test_utils::check_agent_boom;
    use kitsune_p2p_types::config::TransportConfig;
    use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;

    // Get the env var settings for number of simulated agents and
    // the minimum number of ops that should be held by each agent
    // if there are some or use defaults.
    let (num_agents, min_ops) = std::env::var_os("NUM_AGENTS")
        .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
        .and_then(|na| {
            std::env::var_os("MIN_OPS")
                .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
                .map(|mo| (na, mo))
        })
        .unwrap_or((100, 10));

    // Check if we should for new data to be generated even if it already exists.
    let force_new_data = std::env::var_os("FORCE_NEW_DATA").is_some();

    let _g = holochain_trace::test_run();

    // Generate or use cached test data.
    let (data, mut conn) = generate_test_data(num_agents, min_ops, false, force_new_data).await;
    let data = Arc::new(data);

    // We have to use the same dna that was used to generate the test data.
    // This is a short coming I hope to overcome in future versions.
    let dna_file = data_zome(data.integrity_uuid.clone(), data.coordinator_uuid.clone()).await;

    // We are pretending that all simulated agents have all other agents (except the real agent)
    // for this test.

    // Create the one agent bloom.

    // Create the simulated network.
    let (from_kitsune_tx, to_kitsune_rx, mut channel) = HolochainP2pMockChannel::channel(
        // Pass in the generated simulated peer data.
        data.agent_info().cloned().collect(),
        // We want to buffer up to 1000 network messages.
        1000,
        MockScenario {
            // Don't drop any individual messages.
            percent_drop_msg: 0.0,
            // A random 10% of simulated agents will never be online.
            percent_offline: 0.0,
            // Simulated agents will receive messages from within 50 to 100 ms.
            inbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
            // Simulated agents will send messages from within 50 to 100 ms.
            outbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
        },
    );

    // Share alice's peer data as it changes.
    let alice_info = Arc::new(parking_lot::Mutex::new(None));

    let num_gets = Arc::new(AtomicUsize::new(0));
    let num_misses = Arc::new(AtomicUsize::new(0));

    // Spawn the task that will simulate the network.
    tokio::task::spawn({
        let data = data.clone();
        let num_gets = num_gets.clone();
        let num_misses = num_misses.clone();
        async move {
            let mut last_intervals = None;

            // Get the next network message.
            while let Some((msg, respond)) = channel.next().await {
                let AddressedHolochainP2pMockMsg { agent, msg } = msg;
                let agent = Arc::new(agent);

                // Match on the message and create a response (if a response is needed).
                match msg {
                    HolochainP2pMockMsg::Wire { msg, .. } => match msg {
                        holochain_p2p::WireMessage::CallRemoteMulti { .. } => {
                            debug!("CallRemoteMulti")
                        }
                        holochain_p2p::WireMessage::CallRemote { .. } => debug!("CallRemote"),
                        holochain_p2p::WireMessage::ValidationReceipts { receipts: _ } => {
                            debug!("Validation Receipt")
                        }
                        holochain_p2p::WireMessage::Get { dht_hash, options } => {
                            num_gets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let ops = if data.agent_to_arq[&agent]
                                .to_dht_arc_range_std()
                                .contains(dht_hash.get_loc())
                            {
                                let txn = conn
                                    .transaction_with_behavior(
                                        rusqlite::TransactionBehavior::Exclusive,
                                    )
                                    .unwrap();
                                let ops = holochain_cascade::test_utils::handle_get_txn(
                                    &txn,
                                    dht_hash.clone(),
                                    options,
                                );
                                match &ops {
                                    WireOps::Record(WireRecordOps { action, .. }) => {
                                        if action.is_some() {
                                            // eprintln!("Got get hit!");
                                        } else {
                                            eprintln!("Data is missing!");
                                        }
                                    }
                                    _ => (),
                                }
                                ops
                            } else {
                                num_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                // eprintln!("Get sent to wrong agent!");
                                WireOps::Record(WireRecordOps::default())
                            };
                            let ops: Vec<u8> =
                                UnsafeBytes::from(SerializedBytes::try_from(ops).unwrap()).into();
                            let msg = HolochainP2pMockMsg::CallResp(ops.into());
                            respond.unwrap().respond(msg);
                        }
                        holochain_p2p::WireMessage::GetMeta { .. } => debug!("get_meta"),
                        holochain_p2p::WireMessage::GetLinks { .. } => debug!("get_links"),
                        holochain_p2p::WireMessage::CountLinks { .. } => debug!("count_links"),
                        holochain_p2p::WireMessage::GetAgentActivity { .. } => {
                            debug!("get_agent_activity")
                        }
                        holochain_p2p::WireMessage::MustGetAgentActivity { .. } => {
                            debug!("must_get_agent_activity")
                        }
                        holochain_p2p::WireMessage::CountersigningSessionNegotiation { .. } => {
                            debug!("countersigning_session_negotiation")
                        }
                        holochain_p2p::WireMessage::PublishCountersign { .. } => {
                            debug!("publish_countersign")
                        }
                    },
                    HolochainP2pMockMsg::CallResp(_) => debug!("CallResp"),
                    HolochainP2pMockMsg::MetricExchange(_) => debug!("MetricExchange"),
                    HolochainP2pMockMsg::PeerGet(_) => eprintln!("PeerGet"),
                    HolochainP2pMockMsg::PeerGetResp(_) => debug!("PeerGetResp"),
                    HolochainP2pMockMsg::PeerUnsolicited(_) => debug!("PeerUnsolicited"),
                    HolochainP2pMockMsg::PeerQuery(kitsune_p2p::wire::PeerQuery {
                        basis_loc,
                        ..
                    }) => {
                        let this_arc = data.agent_to_arq[&agent];
                        let basis_loc_i = basis_loc.as_u32() as i64;
                        let mut agents = data
                            .agent_to_arq
                            .iter()
                            .filter(|(a, _)| this_arc.to_dht_arc_range_std().contains(a.get_loc()))
                            .map(|(a, arc)| {
                                (
                                    if arc.to_dht_arc_range_std().contains(basis_loc) {
                                        0
                                    } else {
                                        (arc.start_loc().as_u32() as i64 - basis_loc_i).abs()
                                    },
                                    a,
                                )
                            })
                            .collect::<Vec<_>>();
                        agents.sort_unstable_by_key(|(d, _)| *d);
                        let agents: Vec<_> = agents
                            .into_iter()
                            .take(8)
                            .map(|(_, a)| data.agent_to_info[a].clone())
                            .collect();
                        eprintln!("PeerQuery returned {}", agents.len());
                        let msg =
                            HolochainP2pMockMsg::PeerQueryResp(kitsune_p2p::wire::PeerQueryResp {
                                peer_list: agents,
                            });
                        respond.unwrap().respond(msg);
                    }
                    HolochainP2pMockMsg::PeerQueryResp(_) => debug!("PeerQueryResp"),
                    HolochainP2pMockMsg::Gossip {
                        dna,
                        module,
                        gossip,
                    } => {
                        if let kitsune_p2p::GossipModuleType::ShardedRecent = module {
                            #[allow(irrefutable_let_patterns)]
                            if let GossipProtocol::Sharded(gossip) = gossip {
                                use kitsune_p2p::gossip::sharded_gossip::*;
                                match gossip {
                                    ShardedGossipWire::Initiate(Initiate { intervals, .. }) => {
                                        // Capture the intervals from alice.
                                        // This works because alice will only initiate with one simulated
                                        // agent at a time.
                                        last_intervals = Some(intervals);
                                        let arc = data.agent_to_arq[&agent];
                                        let agent_info = data.agent_to_info[&agent].clone();
                                        let interval = arc;

                                        // Accept the initiate.
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(
                                                    vec![interval.to_bounds_std()],
                                                    vec![agent_info],
                                                ),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

                                        // Create an ops bloom and send it back.
                                        let window = (Timestamp::now()
                                            - std::time::Duration::from_secs(60 * 60))
                                        .unwrap()
                                            ..Timestamp::now();
                                        let this_agent_hashes: Vec<_> = data
                                            .hashes_authority_for(&agent)
                                            .into_iter()
                                            .filter(|h| {
                                                window.contains(&data.ops[h].action().timestamp())
                                            })
                                            .map(|k| data.op_hash_to_kit[&k].clone())
                                            .collect();
                                        let filter = if this_agent_hashes.is_empty() {
                                            EncodedTimedBloomFilter::MissingAllHashes {
                                                time_window: window,
                                            }
                                        } else {
                                            let filter =
                                                test_utils::create_op_bloom(this_agent_hashes);

                                            EncodedTimedBloomFilter::HaveHashes {
                                                time_window: window,
                                                filter,
                                            }
                                        };
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::op_bloom(filter, true),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

                                        // Create an agent bloom and send it.
                                        let agent_bloom = create_agent_bloom(
                                            data.agent_info(),
                                            Some(&data.agent_to_info[&agent]),
                                        );
                                        if let Some(agent_bloom) = agent_bloom {
                                            let msg = HolochainP2pMockMsg::Gossip {
                                                dna: dna.clone(),
                                                module: module,
                                                gossip: GossipProtocol::Sharded(
                                                    ShardedGossipWire::agents(agent_bloom),
                                                ),
                                            };
                                            channel.send(msg.addressed((*agent).clone())).await;
                                        }
                                    }
                                    ShardedGossipWire::OpBloom(OpBloom {
                                        missing_hashes, ..
                                    }) => {
                                        // We have received an ops bloom so we can respond with any missing
                                        // hashes if there are nay.
                                        let this_agent_hashes = data.hashes_authority_for(&agent);
                                        let hashes = this_agent_hashes.iter().map(|h| {
                                            (
                                                data.ops[h].action().timestamp(),
                                                &data.op_hash_to_kit[h],
                                            )
                                        });

                                        let missing_hashes =
                                            check_ops_bloom(hashes, missing_hashes);
                                        let missing_hashes = match &last_intervals {
                                            Some(intervals) => missing_hashes
                                                .into_iter()
                                                .filter(|hash| {
                                                    intervals[0].to_dht_arc_range_std().contains(
                                                        data.op_to_loc[&data.op_kit_to_hash[*hash]],
                                                    )
                                                })
                                                .cloned()
                                                .collect(),
                                            None => vec![],
                                        };

                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna,
                                            module,
                                            gossip: GossipProtocol::Sharded(
                                                // TODO: get and send sizes for recent gossip ops
                                                ShardedGossipWire::missing_op_hashes(
                                                    missing_hashes
                                                        .into_iter()
                                                        .map(|h| kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized::new(h, None))
                                                        .collect(),
                                                    2,
                                                ),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::Agents(Agents { filter }) => {
                                        let this_agent_arc = &data.agent_to_arq[&agent];
                                        let iter = data
                                            .agent_to_info
                                            .iter()
                                            .filter(|(a, _)| {
                                                this_agent_arc
                                                    .to_dht_arc_range_std()
                                                    .contains(a.get_loc())
                                            })
                                            .map(|(a, info)| (&data.agent_hash_to_kit[a], info));
                                        let agents = check_agent_boom(iter, &filter);
                                        let peer_data = agents
                                            .into_iter()
                                            .map(|a| {
                                                Arc::new(
                                                    data.agent_to_info[&data.agent_kit_to_hash[a]]
                                                        .clone(),
                                                )
                                            })
                                            .collect();
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna,
                                            module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::missing_agents(peer_data),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::OpRegions(_) => todo!("must implement"),

                                    ShardedGossipWire::MissingAgents(_) => {}
                                    ShardedGossipWire::Accept(_) => (),
                                    ShardedGossipWire::MissingOpHashes(_) => (),
                                    ShardedGossipWire::NoAgents(_) => (),
                                    ShardedGossipWire::AlreadyInProgress(_) => (),
                                    ShardedGossipWire::Busy(_) => (),
                                    ShardedGossipWire::Error(_) => (),
                                    ShardedGossipWire::OpBatchReceived(_) => (),
                                }
                            }
                        }
                    }
                    HolochainP2pMockMsg::Failure(reason) => panic!("Failure: {}", reason),
                    HolochainP2pMockMsg::PublishedAgentInfo { .. } => todo!(),
                }
            }
        }
    });

    // Create the mock network.
    let mock_network =
        kitsune_p2p::test_util::mock_network::mock_network(from_kitsune_tx, to_kitsune_rx);
    let mock_network: AdapterFactory = Arc::new(mock_network);

    // Setup the bootstrap.
    let (bootstrap, _shutdown) = run_bootstrap(data.agent_to_info.values().cloned()).await;
    // Setup the network.
    let mut tuning = KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_dynamic_arcs = true;

    let mut network = KitsuneP2pConfig::default();
    network.bootstrap_service = Some(bootstrap);
    network.transport_pool = vec![TransportConfig::Mock {
        mock_network: mock_network.into(),
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::empty();
    config.network = network;

    // Add it to the conductor builder.
    let builder = ConductorBuilder::new().config(config);
    let mut conductor = SweetConductor::from_builder(builder).await;

    // Install the real agent alice.
    let apps = conductor.setup_app("app", [&dna_file]).await.unwrap();

    let (alice,) = apps.into_tuple();
    let alice_p2p_agents_db = conductor.get_p2p_db(alice.cell_id().dna_hash());
    let alice_kit = alice.agent_pubkey().to_kitsune();

    // Spawn a task to update alice's agent info.
    tokio::spawn({
        let alice_info = alice_info.clone();
        async move {
            loop {
                let info = alice_p2p_agents_db.p2p_get_agent(&alice_kit).await.unwrap();

                {
                    if let Some(info) = &info {
                        eprintln!("Alice coverage {:.2}", info.storage_arc().coverage());
                    }
                    *alice_info.lock() = info;
                }

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    let num_actions = data.ops.len();
    loop {
        let mut count = 0;

        for (_i, hash) in data
            .ops
            .values()
            .map(|op| ActionHash::with_data_sync(&op.action()))
            .enumerate()
        {
            let record: Option<Record> = conductor.call(&alice.zome("zome1"), "read", hash).await;
            if record.is_some() {
                count += 1;
            }
            // let gets = num_gets.load(std::sync::atomic::Ordering::Relaxed);
            // let misses = num_misses.load(std::sync::atomic::Ordering::Relaxed);
            // eprintln!(
            //     "checked {:.2}%, got {:.2}%, missed {:.2}%",
            //     i as f64 / num_actions as f64 * 100.0,
            //     count as f64 / num_actions as f64 * 100.0,
            //     misses as f64 / gets as f64 * 100.0
            // );
        }
        eprintln!(
            "DONE got {:.2}%, {} out of {}",
            count as f64 / num_actions as f64 * 100.0,
            count,
            num_actions
        );

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

*/
