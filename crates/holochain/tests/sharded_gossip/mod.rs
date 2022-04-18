use std::sync::Arc;

use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::consistency_10s;
use holochain_p2p::dht_arc::DhtLocation;
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::config::RECENT_THRESHOLD_DEFAULT;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

fn make_config(recent_threshold: Option<u64>) -> ConductorConfig {
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_peer_on_success_next_gossip_delay_ms = 500;
    tuning.danger_gossip_recent_threshold_secs =
        recent_threshold.unwrap_or(RECENT_THRESHOLD_DEFAULT.as_secs());

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
    use holochain::{
        conductor::handle::DevSettingsDelta, test_utils::inline_zomes::simple_create_read_zome,
    };

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, make_config(None)).await;
    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;
    let all_cells = vec![&alice, &bobbo];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;
    // let p2p = conductors[0].envs().p2p().lock().values().next().cloned().unwrap();
    // holochain_state::prelude::dump_tmp(&p2p);
    // holochain_state::prelude::dump_tmp(&alice.env());
    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_gossip_high_data() -> anyhow::Result<()> {
    use holochain::{
        conductor::handle::DevSettingsDelta, test_utils::inline_zomes::batch_create_zome,
    };

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors =
        SweetConductorBatch::from_config(NUM_CONDUCTORS, make_config(Some(0))).await;
    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }

    let (mut dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", batch_create_zome())
        .await
        .unwrap();

    let dna_file = {
        let mut dna = dna_file.dna.into_content();
        dna.topology = Topology {
            space: Dimension::standard_space(),
            time: Dimension::time_quantum_one_second(),
            time_origin: Timestamp::now(),
        };
        dna_file.dna = DnaDefHashed::from_content_sync(dna);
        dna_file
    };

    let apps = conductors
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), _) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hashes: Vec<HeaderHash> = conductors[0]
        .call(&alice.zome("zome1"), "create_batch", 5)
        .await;
    let all_cells = vec![&alice, &bobbo];

    let regions = conductors[0]
        .get_spaces()
        .handle_fetch_op_regions(dna_file.dna(), holochain_p2p::dht_arc::DhtArcSet::Full)
        .await
        .unwrap();
    dbg!(regions.nonzero_regions().collect::<Vec<_>>());

    let alice_ops: BTreeSet<_> = conductors[0]
        .get_spaces()
        .handle_fetch_op_data_by_regions(
            dna_file.dna_hash(),
            vec![holochain_p2p::dht::region::RegionBounds::new(
                (DhtLocation::MIN, DhtLocation::MAX),
                (Timestamp::MIN, Timestamp::MAX),
            )],
        )
        .await
        .unwrap()
        .into_iter()
        .map(kitsune_p2p_types::combinators::first)
        .collect();

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;

    let bobbo_ops: BTreeSet<_> = conductors[1]
        .get_spaces()
        .handle_fetch_op_data_by_regions(
            dna_file.dna_hash(),
            vec![holochain_p2p::dht::region::RegionBounds::new(
                (DhtLocation::MIN, DhtLocation::MAX),
                (Timestamp::MIN, Timestamp::MAX),
            )],
        )
        .await
        .unwrap()
        .into_iter()
        .map(kitsune_p2p_types::combinators::first)
        .collect();

    dbg!(&alice_ops, &bobbo_ops);
    assert_eq!(alice_ops, bobbo_ops);

    // let p2p = conductors[0].envs().p2p().lock().values().next().cloned().unwrap();
    // holochain_state::prelude::dump_tmp(&p2p);
    // holochain_state::prelude::dump_tmp(&alice.env());
    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[1]
        .call(&bobbo.zome("zome1"), "read", hashes[0].clone())
        .await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert!(matches!(
        *element.entry(),
        ElementEntry::Present(Entry::App(_))
    ));

    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_local_gossip() -> anyhow::Result<()> {
    use holochain::{
        conductor::handle::DevSettingsDelta, sweettest::SweetConductor,
        test_utils::inline_zomes::simple_create_read_zome,
    };

    let _g = observability::test_run().ok();

    let mut conductor = SweetConductor::from_config(make_config(None)).await;
    conductor.update_dev_settings(DevSettingsDelta {
        publish: Some(false),
        ..Default::default()
    });

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let alice = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = alice.into_tuple();
    let bobbo = conductor.setup_app("app2 ", &[dna_file]).await.unwrap();

    let (bobbo,) = bobbo.into_tuple();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductor.call(&alice.zome("zome1"), "create", ()).await;
    let all_cells = vec![&alice, &bobbo];

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductor.call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[cfg(feature = "test_utils")]
#[cfg(feature = "TO-BE-REMOVED")]
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

    use holochain_p2p::mock_network::{GossipProtocol, MockScenario};
    use holochain_p2p::{
        dht_arc::DhtArcSet,
        mock_network::{
            AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg,
        },
    };
    use holochain_p2p::{dht_arc::DhtLocation, AgentPubKeyExt};
    use holochain_sqlite::db::AsP2pStateTxExt;
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
    let dna_file = data_zome(data.uuid.clone()).await;

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
                        holochain_p2p::WireMessage::GetValidationPackage { .. } => {
                            debug!("get_validation_package")
                        }
                        holochain_p2p::WireMessage::CountersigningAuthorityResponse { .. } => {
                            debug!("countersigning_authority_response")
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
                                                window.contains(&data.ops[h].header().timestamp())
                                            })
                                            .map(|k| data.op_hash_to_kit[&k].clone())
                                            .collect();
                                        let filter = if this_agent_hashes.is_empty() {
                                            EncodedTimedBloomFilter::MissingAllHashes {
                                                time_window: window,
                                            }
                                        } else {
                                            let filter = create_ops_bloom(this_agent_hashes);

                                            EncodedTimedBloomFilter::HaveHashes {
                                                time_window: window,
                                                filter,
                                            }
                                        };
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::op_blooms(filter, true),
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
                                    ShardedGossipWire::OpBlooms(OpBlooms {
                                        missing_hashes,
                                        ..
                                    }) => {
                                        // We have received an ops bloom so we can respond with any missing
                                        // hashes if there are nay.
                                        let this_agent_hashes = data.hashes_authority_for(&agent);
                                        let num_this_agent_hashes = this_agent_hashes.len();
                                        let hashes = this_agent_hashes.iter().map(|h| {
                                            (
                                                data.ops[h].header().timestamp(),
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
                                    ShardedGossipWire::OpBloomsBatchReceived(_) => (),
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
#[cfg(feature = "TO-BE-REMOVED")]
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

    use holochain_p2p::mock_network::{
        AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg,
    };
    use holochain_p2p::mock_network::{GossipProtocol, MockScenario};
    use holochain_p2p::AgentPubKeyExt;
    use holochain_state::prelude::*;
    use holochain_types::dht_op::WireOps;
    use holochain_types::element::WireElementOps;
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
    let dna_file = data_zome(data.uuid.clone()).await;

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
                                    WireOps::Element(WireElementOps { header, .. }) => {
                                        if header.is_some() {
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
                                WireOps::Element(WireElementOps::default())
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
                        holochain_p2p::WireMessage::GetValidationPackage { .. } => {
                            debug!("get_validation_package")
                        }
                        holochain_p2p::WireMessage::CountersigningAuthorityResponse { .. } => {
                            debug!("countersigning_authority_response")
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
                                                window.contains(&data.ops[h].header().timestamp())
                                            })
                                            .map(|k| data.op_hash_to_kit[&k].clone())
                                            .collect();
                                        let filter = if this_agent_hashes.is_empty() {
                                            EncodedTimedBloomFilter::MissingAllHashes {
                                                time_window: window,
                                            }
                                        } else {
                                            let filter = create_ops_bloom(this_agent_hashes);

                                            EncodedTimedBloomFilter::HaveHashes {
                                                time_window: window,
                                                filter,
                                            }
                                        };
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::op_blooms(filter, true),
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
                                    ShardedGossipWire::OpBlooms(OpBlooms {
                                        missing_hashes,
                                        ..
                                    }) => {
                                        // We have received an ops bloom so we can respond with any missing
                                        // hashes if there are nay.
                                        let this_agent_hashes = data.hashes_authority_for(&agent);
                                        let hashes = this_agent_hashes.iter().map(|h| {
                                            (
                                                data.ops[h].header().timestamp(),
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
                                    ShardedGossipWire::OpBloomsBatchReceived(_) => (),
                                }
                            }
                        }
                    }
                    HolochainP2pMockMsg::Failure(reason) => panic!("Failure: {}", reason),
                    HolochainP2pMockMsg::MetricExchange(_) => (),
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
    let bootstrap = run_bootstrap(data.agent_to_info.values().cloned()).await;
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

    let num_headers = data.ops.len();
    loop {
        let mut count = 0;

        for (_i, hash) in data
            .ops
            .values()
            .map(|op| HeaderHash::with_data_sync(&op.header()))
            .enumerate()
        {
            let element: Option<Element> = conductor.call(&alice.zome("zome1"), "read", hash).await;
            if element.is_some() {
                count += 1;
            }
            // let gets = num_gets.load(std::sync::atomic::Ordering::Relaxed);
            // let misses = num_misses.load(std::sync::atomic::Ordering::Relaxed);
            // eprintln!(
            //     "checked {:.2}%, got {:.2}%, missed {:.2}%",
            //     i as f64 / num_headers as f64 * 100.0,
            //     count as f64 / num_headers as f64 * 100.0,
            //     misses as f64 / gets as f64 * 100.0
            // );
        }
        eprintln!(
            "DONE got {:.2}%, {} out of {}",
            count as f64 / num_headers as f64 * 100.0,
            count,
            num_headers
        );

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

#[cfg(feature = "test_utils")]
#[cfg(feature = "TO-BE-REMOVED")]
async fn run_bootstrap(peer_data: impl Iterator<Item = AgentInfoSigned>) -> Url2 {
    let mut url = url2::url2!("http://127.0.0.1:0");
    let (driver, addr) = kitsune_p2p_bootstrap::run(([127, 0, 0, 1], 0), vec![])
        .await
        .unwrap();
    tokio::spawn(driver);
    let client = reqwest::Client::new();
    url.set_port(Some(addr.port())).unwrap();
    for info in peer_data {
        let _: Option<()> = do_api(url.clone(), "put", info, &client).await.unwrap();
    }
    url
}

#[cfg(feature = "TO-BE-REMOVED")]
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
