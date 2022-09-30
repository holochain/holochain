use std::sync::Arc;
use std::time::Instant;

use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain::conductor::config::ConductorConfig;
use holochain::conductor::handle::DevSettingsDelta;
use holochain::sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile, SweetInlineZomes};
use holochain::test_utils::inline_zomes::{batch_create_zome, simple_crud_zome};
use holochain::test_utils::inline_zomes::{simple_create_read_zome, AppString};
use holochain::test_utils::network_simulation::{data_zome, generate_test_data};
use holochain::test_utils::{consistency_10s, consistency_60s, consistency_60s_advanced};
use holochain::{
    conductor::ConductorBuilder, test_utils::consistency::local_machine_session_with_hashes,
};
use holochain_p2p::*;
use holochain_sqlite::db::*;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::gossip::sharded_gossip::test_utils::{check_ops_boom, create_agent_bloom};
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::config::RECENT_THRESHOLD_DEFAULT;

fn make_config(recent: bool, historical: bool, recent_threshold: Option<u64>) -> ConductorConfig {
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.disable_recent_gossip = !recent;
    tuning.disable_historical_gossip = !historical;
    tuning.danger_gossip_recent_threshold_secs =
        recent_threshold.unwrap_or(RECENT_THRESHOLD_DEFAULT.as_secs());
    tuning.gossip_inbound_target_mbps = 10000.0;
    tuning.gossip_outbound_target_mbps = 10000.0;
    tuning.gossip_historic_outbound_target_mbps = 10000.0;
    tuning.gossip_historic_inbound_target_mbps = 10000.0;
    // tuning.gossip_max_batch_size = 32_000_000;

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);
    config
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_gossip() -> anyhow::Result<()> {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors =
        SweetConductorBatch::from_config(NUM_CONDUCTORS, make_config(true, true, None)).await;
    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;
    let all_cells = vec![&alice, &bobbo];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;
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
async fn fullsync_sharded_gossip_high_data() -> anyhow::Result<()> {
    // let _g = observability::test_run().ok();

    const NUM_CONDUCTORS: usize = 3;
    const NUM_OPS: usize = 100;

    let mut conductors =
        SweetConductorBatch::from_config(NUM_CONDUCTORS, make_config(false, true, Some(0))).await;
    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("zome", batch_create_zome())).await;

    let apps = conductors
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hashes: Vec<ActionHash> = conductors[0]
        .call(&alice.zome("zome"), "create_batch", NUM_OPS)
        .await;
    let all_cells = vec![&alice, &bobbo, &carol];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;

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

/// Test that a gossip payload larger than the max frame size does not
/// cause problems
#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn test_gossip_shutdown() {
    observability::test_run().ok();
    let mut conductors = SweetConductorBatch::from_config(2, make_config(true, true, None)).await;

    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((cell_0,), (cell_1,)) = apps.into_tuples();
    let zome_0 = cell_0.zome(SweetInlineZomes::COORDINATOR);
    let zome_1 = cell_1.zome(SweetInlineZomes::COORDINATOR);

    let hash: ActionHash = conductors[0]
        .call(&zome_0, "create_string", "hi".to_string())
        .await;

    // Test that gossip doesn't happen within 3 seconds (assuming it will never happen)
    conductors[0].shutdown().await;

    conductors.exchange_peer_info().await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert!(record.is_none());

    // Ensure that gossip loops resume upon startup
    conductors[0].startup().await;

    consistency_10s(&[&cell_0, &cell_1]).await;
    let record: Option<Record> = conductors[1].call(&zome_1, "read", hash.clone()).await;
    assert_eq!(record.unwrap().action_address(), &hash);
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn three_way_gossip_recent() {
    observability::test_run().ok();
    let config = make_config(true, false, None);
    three_way_gossip(config).await;
}

#[cfg(feature = "slow_tests")]
#[tokio::test(flavor = "multi_thread")]
async fn three_way_gossip_historical() {
    observability::test_run().ok();
    let config = make_config(false, true, Some(0));
    three_way_gossip(config).await;
}

/// Test that:
/// - 30MB of data can pass from node A to B,
/// - then A can shut down and C and start up,
/// - and then that same data passes from B to C.
async fn three_way_gossip(config: ConductorConfig) {
    let mut conductors = SweetConductorBatch::from_config(2, config.clone()).await;
    let start = Instant::now();

    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let cells: Vec<_> = futures::future::join_all(conductors.iter_mut().map(|c| async {
        let (cell,) = c.setup_app("app", [&dna_file]).await.unwrap().into_tuple();
        cell
    }))
    .await;

    let zomes: Vec<_> = cells
        .iter()
        .map(|c| c.zome(SweetInlineZomes::COORDINATOR))
        .collect();

    let size = 15_000_000;
    let num = 2;

    let mut hashes = vec![];
    for i in 0..num {
        let bytes = vec![42u8 + i as u8; size];
        let hash: ActionHash = conductors[0].call(&zomes[0], "create_bytes", bytes).await;
        hashes.push(hash);
        dbg!(start.elapsed());
    }

    conductors.exchange_peer_info().await;
    consistency_60s(&[&cells[0], &cells[1]]).await;

    tracing::info!(
        "CONSISTENCY REACHED between first two nodes in {:?}",
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
            .filter_map(|(i, r)| r.is_none().then(|| i))
            .collect::<Vec<_>>()
    );
    assert_eq!(records_0, records_1);
    dbg!(start.elapsed());

    conductors[0].shutdown().await;

    // Bring a third conductor online
    let mut conductor = SweetConductor::from_config(config).await;
    let (cell,) = conductor
        .setup_app("app", [&dna_file])
        .await
        .unwrap()
        .into_tuple();
    let zome = cell.zome(SweetInlineZomes::COORDINATOR);

    conductors.add_conductor(conductor);
    conductors.exchange_peer_info().await;

    consistency_60s_advanced(&[(&cells[0], false), (&cells[1], true), (&cell, true)]).await;

    dbg!(start.elapsed());

    let records_2: Vec<Option<Record>> = conductors[2].call(&zome, "read_multi", hashes).await;
    assert_eq!(records_2, records_1);
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_local_gossip() -> anyhow::Result<()> {
    use holochain::{
        conductor::handle::DevSettingsDelta, sweettest::SweetConductor,
        test_utils::inline_zomes::simple_create_read_zome,
    };

    let _g = observability::test_run().ok();

    let mut conductor = SweetConductor::from_config(make_config(true, true, None)).await;
    conductor.update_dev_settings(DevSettingsDelta {
        publish: Some(false),
        ..Default::default()
    });

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let alice = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = alice.into_tuple();
    let bobbo = conductor.setup_app("app2 ", &[dna_file]).await.unwrap();

    let (bobbo,) = bobbo.into_tuple();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor.call(&alice.zome("simple"), "create", ()).await;
    let all_cells = vec![&alice, &bobbo];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;

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
    use kitsune_p2p::TransportConfig;
    use kitsune_p2p::*;
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

    let _g = observability::test_run().ok();

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
    let (bad_publish_tx, mut bad_publish_rx) = tokio::sync::oneshot::channel();
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
            let mut bad_publish = Some(bad_publish_tx);
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
                        holochain_p2p::WireMessage::CallRemote { .. } => debug!("CallRemote"),
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
                        holochain_p2p::WireMessage::ValidationReceipt { receipt: _ } => {
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
                                let arc = data.agent_to_arc[&agent];

                                if !arc.contains(dht_hash.get_loc()) {
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
                                        let arc = data.agent_to_arc[&agent];
                                        let agent_info = data.agent_to_info[&agent].clone();
                                        let interval = arc;

                                        // If we have info for alice check the overlap.
                                        if let Some(alice) = &alice {
                                            let a = alice.storage_arc;
                                            let b = interval.clone();
                                            debug!("{}\n{}", a.to_ascii(10), b.to_ascii(10));
                                            let a: DhtArcSet = a.inner().into();
                                            let b: DhtArcSet = b.inner().into();
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
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(
                                                    vec![interval.into()],
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
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::op_bloom(filter, true),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

                                        // Create an agent bloom and send it.
                                        if let Some(ref agent_bloom) = agent_bloom {
                                            let msg = HolochainP2pMockMsg::Gossip {
                                                dna: dna.clone(),
                                                module: module.clone(),
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

                                        let missing_hashes = check_ops_boom(hashes, missing_hashes);
                                        let missing_hashes = match &last_intervals {
                                            Some(intervals) => missing_hashes
                                                .into_iter()
                                                .filter(|hash| {
                                                    intervals[0].contains(
                                                        data.op_to_loc[&data.op_kit_to_hash[*hash]],
                                                    )
                                                })
                                                .collect(),
                                            None => vec![],
                                        };
                                        gossiped_ops.extend(missing_hashes.iter().cloned());

                                        let missing_ops: Vec<_> = missing_hashes
                                            .into_iter()
                                            .map(|h| data.ops[&data.op_kit_to_hash[h]].clone())
                                            .map(|op| {
                                                kitsune_p2p::KitsuneOpData::new(
                                                    holochain_p2p::WireDhtOpData {
                                                        op_data: op.into_content(),
                                                    }
                                                    .encode()
                                                    .unwrap(),
                                                )
                                            })
                                            .collect();
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
                                                let arc = data.agent_to_arc[&agent];
                                                let a = alice.storage_arc;
                                                let b = arc;
                                                let num_should_hold = this_agent_hashes
                                                    .iter()
                                                    .filter(|hash| {
                                                        let loc = data.op_to_loc[*hash];
                                                        alice.storage_arc.contains(loc)
                                                    })
                                                    .count();
                                                (a.overlap_coverage(&b) * 100.0, num_should_hold)
                                            })
                                            .unwrap_or((0.0, 0));

                                        // Print out some stats.
                                        debug!(
                                            "Gossiped with {}, got {} of {} ops, overlap: {:.2}%, max could get {}, {:.2}% done, avg freq of gossip {:?}, est finish in {:?}",
                                            agent,
                                            missing_ops.len(),
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
                                                ShardedGossipWire::missing_ops(
                                                    missing_ops,
                                                    MissingOpsStatus::AllComplete as u8,
                                                ),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::MissingOps(MissingOps { ops, .. }) => {
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
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_dynamic_arcs = true;

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![TransportConfig::Mock {
        mock_network: mock_network.into(),
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    // Add it to the conductor builder.
    let builder = ConductorBuilder::new().config(config);
    let mut conductor = SweetConductor::from_builder(builder).await;

    // Add in all the agent info.
    conductor
        .add_agent_infos(data.agent_to_info.values().cloned().collect())
        .await
        .unwrap();

    // Install the real agent alice.
    let apps = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = apps.into_tuple();
    let alice_p2p_agents_db = conductor.get_p2p_db(alice.cell_id().dna_hash());
    let alice_kit = alice.agent_pubkey().to_kitsune();

    // Spawn a task to update alice's agent info.
    tokio::spawn({
        let alice_info = alice_info.clone();
        async move {
            loop {
                {
                    let mut conn = alice_p2p_agents_db.conn().unwrap();
                    let txn = conn.transaction().unwrap();
                    let info = txn.p2p_get_agent(&alice_kit).unwrap();
                    {
                        *alice_info.lock() = info;
                    }
                }
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
            // if (alice.storage_arc.coverage() - data.coverage()).abs() < 0.01 {
            let hashes_to_be_held = data
                .ops
                .iter()
                .filter_map(|(hash, op)| {
                    let loc = op.dht_basis().get_loc();
                    alice.storage_arc.contains(loc).then(|| (loc, hash.clone()))
                })
                .collect::<Vec<_>>();
            let agents_that_should_be_initiated_with = data
                .agents()
                .filter(|h| alice.storage_arc.contains(h.get_loc()))
                .cloned()
                .collect::<HashSet<_>>();
            num_hashes_alice_should_hold.store(
                hashes_to_be_held.len(),
                std::sync::atomic::Ordering::Relaxed,
            );
            debug!("Alice covers {} and the target coverage is {}. She should hold {} out of {} ops. She should gossip with {} agents", alice.storage_arc.coverage(), data.coverage(), hashes_to_be_held.len(), data.ops.len(), agents_that_should_be_initiated_with.len());
            break (hashes_to_be_held, agents_that_should_be_initiated_with);
            // }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    };

    // Wait for consistency to be reached.
    local_machine_session_with_hashes(
        vec![&conductor.handle()],
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

    // Check if we got any publishes to the wrong agent.
    match bad_publish_rx.try_recv() {
        Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            panic!("Got a bad publish")
        }
        Err(_) => (),
    }

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
    use kitsune_p2p::TransportConfig;
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

    let _g = observability::test_run().ok();

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
                        holochain_p2p::WireMessage::CallRemote { .. } => debug!("CallRemote"),
                        holochain_p2p::WireMessage::Publish { .. } => {}
                        holochain_p2p::WireMessage::ValidationReceipt { receipt: _ } => {
                            debug!("Validation Receipt")
                        }
                        holochain_p2p::WireMessage::Get { dht_hash, options } => {
                            num_gets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let ops = if data.agent_to_arc[&agent].contains(dht_hash.get_loc()) {
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
                    HolochainP2pMockMsg::MetricExchange(_) => debug!("MetricExchange"),
                    HolochainP2pMockMsg::PeerGet(_) => eprintln!("PeerGet"),
                    HolochainP2pMockMsg::PeerGetResp(_) => debug!("PeerGetResp"),
                    HolochainP2pMockMsg::PeerQuery(kitsune_p2p::wire::PeerQuery {
                        basis_loc,
                        ..
                    }) => {
                        let this_arc = data.agent_to_arc[&agent].clone();
                        let basis_loc_i = basis_loc.as_u32() as i64;
                        let mut agents = data
                            .agent_to_arc
                            .iter()
                            .filter(|(a, _)| this_arc.contains(a.get_loc()))
                            .map(|(a, arc)| {
                                (
                                    if arc.contains(basis_loc) {
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
                                        let arc = data.agent_to_arc[&agent];
                                        let agent_info = data.agent_to_info[&agent].clone();
                                        let interval = arc;

                                        // Accept the initiate.
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(
                                                    vec![interval.into()],
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
                                            module: module.clone(),
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
                                                module: module.clone(),
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

                                        let missing_hashes = check_ops_boom(hashes, missing_hashes);
                                        let missing_hashes = match &last_intervals {
                                            Some(intervals) => missing_hashes
                                                .into_iter()
                                                .filter(|hash| {
                                                    intervals[0].contains(
                                                        data.op_to_loc[&data.op_kit_to_hash[*hash]],
                                                    )
                                                })
                                                .collect(),
                                            None => vec![],
                                        };

                                        let missing_ops: Vec<_> = missing_hashes
                                            .into_iter()
                                            .map(|h| data.ops[&data.op_kit_to_hash[h]].clone())
                                            .map(|op| {
                                                kitsune_p2p::KitsuneOpData::new(
                                                    holochain_p2p::WireDhtOpData {
                                                        op_data: op.into_content(),
                                                    }
                                                    .encode()
                                                    .unwrap(),
                                                )
                                            })
                                            .collect();

                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna,
                                            module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::missing_ops(missing_ops, 2),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::Agents(Agents { filter }) => {
                                        let this_agent_arc = &data.agent_to_arc[&agent];
                                        let iter = data
                                            .agent_to_info
                                            .iter()
                                            .filter(|(a, _)| this_agent_arc.contains(a.get_loc()))
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
                                    ShardedGossipWire::MissingOps(_) => (),
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
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_dynamic_arcs = true;

    let mut network = KitsuneP2pConfig::default();
    network.bootstrap_service = Some(bootstrap);
    network.transport_pool = vec![TransportConfig::Mock {
        mock_network: mock_network.into(),
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    // Add it to the conductor builder.
    let builder = ConductorBuilder::new().config(config);
    let mut conductor = SweetConductor::from_builder(builder).await;

    // Install the real agent alice.
    let apps = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = apps.into_tuple();
    let alice_p2p_agents_db = conductor.get_p2p_db(alice.cell_id().dna_hash());
    let alice_kit = alice.agent_pubkey().to_kitsune();

    // Spawn a task to update alice's agent info.
    tokio::spawn({
        let alice_info = alice_info.clone();
        async move {
            loop {
                fresh_reader_test(alice_p2p_agents_db.clone(), |txn| {
                    let info = txn.p2p_get_agent(&alice_kit).unwrap();
                    {
                        if let Some(info) = &info {
                            eprintln!("Alice coverage {:.2}", info.storage_arc.coverage());
                        }
                        *alice_info.lock() = info;
                    }
                });
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
