use std::sync::Arc;

use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::consistency_10s;
use holochain::test_utils::network_simulation::{data_zome, generate_test_data};
use holochain::{
    conductor::ConductorBuilder,
    test_utils::{consistency::local_machine_session_with_hashes, wait_for_integration},
};
use holochain_p2p::mock_network::{GossipProtocol, MockScenario};
use holochain_p2p::{
    dht_arc::DhtArcSet,
    mock_network::{AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg},
};
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::gossip::sharded_gossip::test_utils::create_ops_bloom;
use kitsune_p2p::gossip::sharded_gossip::test_utils::{check_ops_boom, create_agent_bloom};
use kitsune_p2p::KitsuneP2pConfig;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn fullsync_sharded_gossip() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 2;

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;
    for c in conductors.iter() {
        c.set_skip_publish(true);
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
async fn fullsync_sharded_local_gossip() -> anyhow::Result<()> {
    use holochain::{sweettest::SweetConductor, test_utils::inline_zomes::simple_create_read_zome};

    let _g = observability::test_run().ok();

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let mut conductor = SweetConductor::from_config(config).await;
    conductor.set_skip_publish(true);

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
#[tokio::test(flavor = "multi_thread")]
#[ignore = "Prototype test that is not suitable for CI"]
async fn mock_network_sharded_gossip() {
    use holochain_p2p::dht_arc::DhtLocation;

    let (num_agents, min_ops) = std::env::var_os("NUM_AGENTS")
        .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
        .and_then(|na| {
            std::env::var_os("MIN_OPS")
                .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
                .map(|mo| (na, mo))
        })
        .unwrap_or((100, 10));
    let force_new_data = std::env::var_os("FORCE_NEW_DATA").is_some();

    let _g = observability::test_run().ok();

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let (data, mut conn) = generate_test_data(num_agents, min_ops, false, force_new_data).await;
    let data = Arc::new(data);

    #[derive(Debug)]
    struct ExpectedData {
        hashes_to_be_held: Vec<(DhtLocation, Arc<DhtOpHash>)>,
        agents_that_should_be_initiated_with: HashSet<Arc<AgentPubKey>>,
    }

    let dna_file = data_zome(data.uuid.clone()).await;

    let agent_bloom = create_agent_bloom(data.agent_info(), None);
    let (from_kitsune_tx, to_kitsune_rx, mut channel) = HolochainP2pMockChannel::channel(
        data.agent_info().cloned().collect(),
        1000,
        MockScenario {
            percent_drop_msg: 0.0,
            percent_offline: 0.1,
            inbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
            outbound_delay_range: std::time::Duration::from_millis(50)
                ..std::time::Duration::from_millis(150),
        },
    );
    let (ops_to_hold_tx, ops_to_hold_rx) = tokio::sync::oneshot::channel();
    let (bad_publish_tx, mut bad_publish_rx) = tokio::sync::oneshot::channel();
    let (bad_get_tx, mut bad_get_rx) = tokio::sync::oneshot::channel();
    let (agents_gossiped_with_tx, mut agents_gossiped_with_rx) =
        tokio::sync::watch::channel(HashSet::new());
    tokio::task::spawn({
        let data = data.clone();
        async move {
            let mut alice: Option<Arc<AgentInfoSigned>> = None;
            let mut num_hashes_alice_should_hold: usize = 0;
            let mut gossiped_ops = HashSet::new();
            let start_time = std::time::Instant::now();
            let mut ops_to_hold_tx = Some(ops_to_hold_tx);
            let mut agents_gossiped_with = HashSet::new();
            let mut num_missed_gossips = 0;
            let mut last_intervals = None;
            let mut bad_publish = Some(bad_publish_tx);
            let mut bad_get = Some(bad_get_tx);
            while let Some((msg, respond)) = channel.next().await {
                let AddressedHolochainP2pMockMsg { agent, msg } = msg;
                let agent = Arc::new(agent);
                match msg {
                    HolochainP2pMockMsg::Wire { msg, .. } => match msg {
                        holochain_p2p::WireMessage::CallRemote { .. } => debug!("CallRemote"),
                        holochain_p2p::WireMessage::Publish { ops, .. } => {
                            if bad_publish.is_some() {
                                let arc = data.agent_to_arc[&agent];
                                if ops
                                    .into_iter()
                                    .any(|(_, op)| !arc.contains(op.dht_basis().get_loc()))
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
                                        last_intervals = Some(intervals);
                                        let arc = data.agent_to_arc[&agent];
                                        let interval = arc.interval();
                                        if let Some(alice) = &alice {
                                            let a = alice.storage_arc.interval();
                                            let b = interval.clone();
                                            eprintln!("{}\n{}", a.to_ascii(10), b.to_ascii(10));
                                            let a: DhtArcSet = a.into();
                                            let b: DhtArcSet = b.into();
                                            if !a.overlap(&b) {
                                                num_missed_gossips += 1;
                                            }
                                        }
                                        agents_gossiped_with.insert(agent.clone());
                                        agents_gossiped_with_tx
                                            .send(agents_gossiped_with.clone())
                                            .unwrap();
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(vec![interval]),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;

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
                                                ShardedGossipWire::ops(filter, true),
                                            ),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
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
                                    ShardedGossipWire::Ops(Ops { missing_hashes, .. }) => {
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
                                            .map(|h| {
                                                (
                                                    h.clone(),
                                                    data.ops[&data.op_kit_to_hash[h]].clone(),
                                                )
                                            })
                                            .map(|(hash, op)| {
                                                (
                                                    hash,
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
                                                let a = alice.storage_arc.interval();
                                                let b = arc.interval();
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
                                        eprintln!(
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
                                                ShardedGossipWire::missing_ops(missing_ops, true),
                                            ),
                                        };
                                        // TODO: Turn to kitsune type and send back missing hashes.
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                    ShardedGossipWire::MissingOps(MissingOps { ops, .. }) => {
                                        // eprintln!("Missing ops with: {}", agent);
                                        eprintln!(
                                        "Gossiped with {} {} out of {}, who sent {} ops and gossiped with {} nodes outside of arc",
                                        agent,
                                        agents_gossiped_with.len(),
                                        data.num_agents(),
                                        ops.len(),
                                        num_missed_gossips
                                    );
                                    }
                                    ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                                        alice = Some(agents[0].clone());
                                    }
                                    _ => (),
                                }
                            }
                        }
                    }
                    HolochainP2pMockMsg::Failure(_) => todo!(),
                }
                if ops_to_hold_tx.is_some() {
                    if let Some(alice) = &alice {
                        if (alice.storage_arc.coverage() - data.coverage()).abs() < 0.01 {
                            let hashes_that_should_be_held = data
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
                            eprintln!("Alice covers {} and the target coverage is {}. She should hold {} out of {} ops. She should gossip with {} agents", alice.storage_arc.coverage(), data.coverage(), hashes_that_should_be_held.len(), data.ops.len(), agents_that_should_be_initiated_with.len());
                            num_hashes_alice_should_hold = hashes_that_should_be_held.len();
                            let msg = ExpectedData {
                                hashes_to_be_held: hashes_that_should_be_held,
                                agents_that_should_be_initiated_with,
                            };
                            ops_to_hold_tx.take().unwrap().send(msg).unwrap();
                        }
                    }
                }
            }
        }
    });
    let mock_network =
        kitsune_p2p::test_util::mock_network::mock_network(from_kitsune_tx, to_kitsune_rx);
    let builder = ConductorBuilder::new()
        .config(config)
        .with_mock_p2p(mock_network);
    let mut conductor = SweetConductor::from_builder(builder).await;

    conductor
        .add_agent_infos(data.agent_to_info.values().cloned().collect())
        .await
        .unwrap();
    let apps = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = apps.into_tuple();
    let ExpectedData {
        hashes_to_be_held,
        agents_that_should_be_initiated_with,
    } = ops_to_hold_rx.await.unwrap();

    local_machine_session_with_hashes(
        vec![&conductor.handle()],
        hashes_to_be_held.iter().map(|(l, h)| (*l, (**h).clone())),
        dna_file.dna_hash(),
        std::time::Duration::from_secs(60 * 60),
    )
    .await;
    wait_for_integration(
        alice.env(),
        hashes_to_be_held.len(),
        1_000_000,
        std::time::Duration::from_millis(500),
    )
    .await;

    while agents_gossiped_with_rx.changed().await.is_ok() {
        let new = agents_gossiped_with_rx.borrow();
        let diff: Vec<_> = agents_that_should_be_initiated_with
            .difference(&new)
            .collect();
        if diff.is_empty() {
            break;
        } else {
            eprintln!("Waiting for {} to initiated agents", diff.len());
        }
    }

    match bad_publish_rx.try_recv() {
        Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            panic!("Got a bad publish")
        }
        Err(_) => (),
    }
    match bad_get_rx.try_recv() {
        Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            panic!("Got a bad get")
        }
        Err(_) => (),
    }
}
