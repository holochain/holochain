#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::single_match)]

use std::time::{Duration, Instant};

use hdk::prelude::*;
use holochain::sweettest::*;
use holochain::test_utils::inline_zomes::{
    batch_create_zome, simple_create_read_zome, simple_crud_zome,
};
use holochain::test_utils::WaitFor;
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

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn fullsync_sharded_gossip_low_data() -> anyhow::Result<()> {
    holochain_trace::test_run();
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
        .await
        .unwrap();

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
        .await
        .unwrap();

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

    let config_0: SweetConductorConfig = TestConfig {
        publish: true,
        recent: true,
        historical: true,
        bootstrap: false,
        recent_threshold: None,
    }
    .into();

    // Standard config with arc clamped to zero and publishing off
    // This should result in no publishing or gossip
    let mut tuning_1 = make_tuning(false, true, true, None);
    tuning_1.gossip_arc_clamping = "empty".into();
    let config_0 = config_0.no_dpki_mustfix();
    let config_1 = SweetConductorConfig::rendezvous(false)
        // Zero-arc nodes can't use DPKI in this test,
        // since they can't learn about other peers' keys,
        // since publishing was turned off.
        .no_dpki_mustfix()
        .set_tuning_params(tuning_1);

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
async fn test_zero_arc_no_gossip_4way() {
    use futures::future::join_all;
    use maplit::hashset;

    holochain_trace::test_run();

    // XXX: We disable DPKI for this test just because it's so slow for 4 conductors.

    let configs = [
        // Standard config
        <TestConfig as Into<SweetConductorConfig>>::into(TestConfig {
            publish: true,
            recent: true,
            historical: true,
            bootstrap: false,
            recent_threshold: None,
        })
        .no_dpki()
        .tune_conductor(|params| {
            // Speed up sys validation retry when gets hit a conductor that isn't yet serving the requested data
            params.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
        }),
        // Publishing turned off
        <TestConfig as Into<SweetConductorConfig>>::into(TestConfig {
            publish: false,
            recent: true,
            historical: true,
            bootstrap: false,
            recent_threshold: None,
        })
        .no_dpki()
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
                .no_dpki()
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
                .no_dpki()
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
        let stored_agents: HashSet<_> = holochain::conductor::p2p_agent_store::all_agent_infos(
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
        .collect();

        let expected = hashset![
            // conductors[i].dpki_cell().unwrap().agent_pubkey().clone(),
            cell.agent_pubkey().clone()
        ];

        assert_eq!(stored_agents, expected,);
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
            bootstrap: false,
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
    let config = || async move {
        SweetConductorConfig::standard().tune(|t| {
            t.danger_gossip_recent_threshold_secs = 1;
            t.default_rpc_single_timeout_ms = 3_000;
        })
    };

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mk_conductor = || async {
        let cfg = config().await;
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

    await_consistency(60, [&cell0, &cell1]).await.unwrap();
    let record: Option<Record> = conductor1.call(&zome1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn three_way_gossip_recent() {
    holochain_trace::test_run();

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
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn three_way_gossip_historical() {
    holochain_trace::test_run();

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
    // TODO: this fails miserably with DPKI enabled. Why?
    let config = config.no_dpki_mustfix();

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config.clone()).await;
    let start = Instant::now();

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let cells = conductors
        .setup_app("app", [&dna_file])
        .await
        .unwrap()
        .cells_flattened();

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
    let num = 3;

    let mut hashes = vec![];
    for i in 0..num {
        let bytes = vec![42u8 + i as u8; size];
        let hash: ActionHash = conductors[0].call(&zomes[0], "create_bytes", bytes).await;
        hashes.push(hash);
    }

    await_consistency(10, [&cells[0], &cells[1]]).await.unwrap();

    let dump0 = conductors[0]
        .dump_all_integrated_op_hashes(cells[0].cell_id().dna_hash())
        .await
        .unwrap();
    let dump1 = conductors[1]
        .dump_all_integrated_op_hashes(cells[1].cell_id().dna_hash())
        .await
        .unwrap();
    pretty_assertions::assert_eq!(dump0, dump1);

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

    // conductors.persist_dbs();

    let (cell,) = conductors[2]
        .setup_app("app", [&dna_file])
        .await
        .unwrap()
        .into_tuple();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);
    SweetConductor::exchange_peer_info([&conductors[1], &conductors[2]]).await;

    println!(
        "Newcomer agent joined: agent={:#?}",
        cell.agent_pubkey().to_kitsune()
    );

    conductors[2]
        .require_initial_gossip_activity_for_cell(&cell, 2, Duration::from_secs(60))
        .await
        .unwrap();

    println!(
        "Initial gossip activity completed. Elapsed: {:?}",
        start.elapsed()
    );

    await_consistency_advanced(
        20,
        (),
        [(&cells[0], false), (&cells[1], true), (&cell, true)],
    )
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

    let dump1 = conductors[1]
        .dump_all_integrated_op_hashes(cells[1].cell_id().dna_hash())
        .await
        .unwrap();
    let dump2 = conductors[2]
        .dump_all_integrated_op_hashes(cell.cell_id().dna_hash())
        .await
        .unwrap();

    pretty_assertions::assert_eq!(dump1, dump2);
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_local_gossip() -> anyhow::Result<()> {
    use holochain::{sweettest::SweetConductor, test_utils::inline_zomes::simple_create_read_zome};

    holochain_trace::test_run();

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
    let bobbo = conductor.setup_app("app2", [&dna_file]).await.unwrap();

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
