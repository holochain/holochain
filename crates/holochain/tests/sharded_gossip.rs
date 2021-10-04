use std::collections::HashMap;
use std::sync::Arc;

use ::fixt::prelude::*;
use hdk::prelude::*;
use holo_hash::{DhtOpHash, DnaHash};
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::consistency_10s;
use holochain_keystore::KeystoreSenderExt;
use holochain_p2p::dht_arc::{ArcInterval, DhtArc};
use holochain_p2p::DnaHashExt;
use holochain_sqlite::conn::DbSyncLevel;
use holochain_state::prelude::from_blob;
use holochain_state::test_utils::fresh_reader_test;
use holochain_types::dht_op::{DhtOp, DhtOpHashed, DhtOpType};
use holochain_types::prelude::DnaFile;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::fixt::*;
use kitsune_p2p::KitsuneP2pConfig;
use rand::distributions::Alphanumeric;
use rand::distributions::Standard;
use rand::Rng;
use std::{collections::BTreeMap, time::Duration};

use holo_hash::HashableContentExtSync;
use holochain::{
    conductor::ConductorBuilder,
    test_utils::{consistency::local_machine_session_with_hashes, wait_for_integration},
};
use holochain_p2p::{
    dht_arc::{DhtArcBucket, DhtArcSet},
    mock_network::{AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg},
    AgentPubKeyExt, DhtOpHashExt,
};
use kitsune_p2p::{
    gossip::sharded_gossip::test_utils::{check_ops_boom, create_agent_bloom},
    KitsuneBinType,
};
use rusqlite::Connection;

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
    use holochain_p2p::mock_network::MockScenario;
    use kitsune_p2p::gossip::sharded_gossip::test_utils::create_ops_bloom;

    let num_agents = std::env::var_os("NUM_AGENTS")
        .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
        .and_then(|na| {
            std::env::var_os("MIN_OPS")
                .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
                .map(|mo| (na, mo))
        });

    let _g = observability::test_run().ok();

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let data = match num_agents {
        Some((num_agents, min_ops)) => generate_test_data(num_agents, min_ops).await,
        None => {
            let p = std::env::temp_dir()
                .join("mock_test_data")
                .join("mock_test_data");
            let data = std::fs::read(p).ok().and_then(|data| {
                SerializedBytes::from(UnsafeBytes::from(data))
                    .try_into()
                    .ok()
            });
            match data {
                Some(data) => data,
                None => generate_test_data(1000, 10).await,
            }
        }
    };

    let MockNetworkData {
        peer_data,
        ops,
        uuid,
    } = data;

    #[derive(Debug)]
    struct ExpectedData {
        hashes_to_be_held: Vec<(u32, Arc<DhtOpHash>)>,
        agents_that_should_be_initiated_with: HashSet<Arc<AgentPubKey>>,
    }

    let mut data_agents_authored = HashMap::with_capacity(ops.len());
    let len = ops.values().map(|op| op.len()).sum::<usize>();
    let mut data = HashMap::with_capacity(len);
    for (agent, ops) in ops {
        let hashes: Vec<_> = ops.iter().map(|op| Arc::new(op.to_hash())).collect();
        data_agents_authored.insert(Arc::new(agent), hashes.clone());
        data.extend(hashes.into_iter().zip(ops.into_iter()));
    }
    let (h_to_k_map, k_to_h_map): (HashMap<_, _>, HashMap<_, _>) = data
        .iter()
        .map(|(hash, _)| {
            let k_hash = hash.to_kitsune();
            (
                (hash.clone(), k_hash.clone()),
                (k_hash.clone(), hash.clone()),
            )
        })
        .unzip();
    let dna_file = data_zome(uuid).await;
    let peer_data = reset_peer_data(peer_data, dna_file.dna_hash()).await;
    let coverage = ((50.0 / peer_data.len() as f64) * 2.0).clamp(0.0, 1.0);

    let (h_to_k_map_agent, k_to_h_map_agent): (HashMap<_, _>, HashMap<_, _>) = data_agents_authored
        .keys()
        .map(|agent| {
            let k_agent = agent.to_kitsune();
            ((agent.clone(), k_agent.clone()), (k_agent, agent.clone()))
        })
        .unzip();

    let mut arcs = Vec::new();
    let mut h_to_arc_map = HashMap::with_capacity(peer_data.len());
    for peer in &peer_data {
        arcs.push(peer.storage_arc);
        h_to_arc_map.insert(
            get_map(&k_to_h_map_agent, peer.agent.as_ref()),
            peer.storage_arc.clone(),
        );

        println!(
            "{}:{}",
            peer.storage_arc.interval().to_ascii(10),
            peer.agent.get_loc()
        );
    }

    let ops_by_loc: BTreeMap<_, _> = h_to_k_map.keys().fold(BTreeMap::new(), |mut map, hash| {
        let loc = get_map_ref(&data, hash).dht_basis().get_loc();
        map.entry(loc).or_insert_with(Vec::new).push(hash.clone());
        map
    });
    let h_to_loc_map: HashMap<_, _> = data
        .iter()
        .map(|(h, op)| (h.clone(), op.dht_basis().get_loc()))
        .collect();

    let ops_each_agent_should_hold: HashMap<_, _> = peer_data
        .iter()
        .map(|peer| {
            let arc = peer.storage_arc.interval();
            let ops = match arc {
                ArcInterval::Empty => Vec::with_capacity(0),
                ArcInterval::Full => ops_by_loc.values().flatten().cloned().collect(),
                ArcInterval::Bounded(start, end) => {
                    if start <= end {
                        ops_by_loc
                            .range(start..=end)
                            .flat_map(|(_, hash)| hash)
                            .cloned()
                            .collect()
                    } else {
                        ops_by_loc
                            .range(..=end)
                            .flat_map(|(_, hash)| hash)
                            .chain(ops_by_loc.range(start..).flat_map(|(_, hash)| hash))
                            .cloned()
                            .collect()
                    }
                }
            };
            (get_map(&k_to_h_map_agent, peer.agent.as_ref()), ops)
        })
        .collect();

    let mut conn = Connection::open_in_memory().unwrap();
    holochain_sqlite::schema::SCHEMA_CELL
        .initialize(&mut conn, None)
        .unwrap();
    let mut txn = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    for op in data.values().cloned() {
        holochain_state::test_utils::mutations_helpers::insert_valid_integrated_op(&mut txn, op)
            .unwrap();
    }
    txn.commit().unwrap();

    let bucket = DhtArcBucket::new(DhtArc::full(0), arcs.clone());
    println!("{}\n{:?}", bucket, bucket.density());
    let bucket = DhtArcBucket::new(DhtArc::with_coverage(0, 0.1), arcs.clone());
    println!("{:?}", bucket.density());
    let bucket = DhtArcBucket::new(DhtArc::with_coverage(u32::MAX / 2, 0.1), arcs.clone());
    println!("{:?}", bucket.density());

    let agent_bloom = create_agent_bloom(peer_data.iter(), None);
    let (from_kitsune_tx, to_kitsune_rx, mut channel) = HolochainP2pMockChannel::channel(
        peer_data.clone(),
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
        let peer_data = peer_data.clone();
        async move {
            let mut alice: Option<Arc<AgentInfoSigned>> = None;
            let mut num_hashes_alice_should_hold: usize = 0;
            let mut gossiped_ops = HashSet::new();
            let mut avg_gossip_freq = Duration::default();
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
                        holochain_p2p::WireMessage::CallRemote { .. } => eprintln!("CallRemote"),
                        holochain_p2p::WireMessage::Publish { ops, .. } => {
                            if bad_publish.is_some() {
                                let arc = get_map(&h_to_arc_map, &agent);
                                if ops
                                    .into_iter()
                                    .any(|(_, op)| !arc.contains(op.dht_basis().get_loc()))
                                {
                                    bad_publish.take().unwrap().send(()).unwrap();
                                }
                            }
                        }
                        holochain_p2p::WireMessage::ValidationReceipt { receipt: _ } => {
                            eprintln!("Validation Receipt")
                        }
                        holochain_p2p::WireMessage::Get { dht_hash, options } => {
                            let txn = conn
                                .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
                                .unwrap();
                            let data = holochain_cascade::test_utils::handle_get_txn(
                                &txn,
                                dht_hash.clone(),
                                options,
                            );
                            if bad_get.is_some() {
                                let arc = get_map(&h_to_arc_map, &agent);

                                if !arc.contains(dht_hash.get_loc()) {
                                    bad_get.take().unwrap().send(()).unwrap();
                                }
                            }
                            // match &data {
                            //     WireOps::Entry(WireEntryOps { creates, entry, .. }) => {
                            //         eprintln!(
                            //             "Handling get entry. Found {} ops and entry {}",
                            //             creates.len(),
                            //             entry.is_some()
                            //         );
                            //     }
                            //     WireOps::Element(WireElementOps { header, entry, .. }) => {
                            //         eprintln!(
                            //             "Handling get element. Found header {} and entry {}",
                            //             header.is_some(),
                            //             entry.is_some()
                            //         );
                            //     }
                            // }
                            let data: Vec<u8> =
                                UnsafeBytes::from(SerializedBytes::try_from(data).unwrap()).into();
                            let msg = HolochainP2pMockMsg::CallResp(data.into());
                            respond.unwrap().respond(msg);
                        }
                        holochain_p2p::WireMessage::GetMeta {
                            dht_hash: _,
                            options: _,
                        } => todo!(),
                        holochain_p2p::WireMessage::GetLinks {
                            link_key: _,
                            options: _,
                        } => todo!(),
                        holochain_p2p::WireMessage::GetAgentActivity {
                            agent: _,
                            query: _,
                            options: _,
                        } => todo!(),
                        holochain_p2p::WireMessage::GetValidationPackage { header_hash: _ } => {
                            todo!()
                        }
                        holochain_p2p::WireMessage::CountersigningAuthorityResponse {
                            signed_headers: _,
                        } => todo!(),
                    },
                    HolochainP2pMockMsg::CallResp(_) => (),
                    HolochainP2pMockMsg::PeerGet(_) => eprintln!("PeerGet"),
                    HolochainP2pMockMsg::PeerGetResp(_) => (),
                    HolochainP2pMockMsg::PeerQuery(_) => eprintln!("PeerQuery"),
                    HolochainP2pMockMsg::PeerQueryResp(_) => (),
                    HolochainP2pMockMsg::Gossip {
                        dna,
                        module,
                        gossip,
                    } => {
                        if let kitsune_p2p::GossipModuleType::ShardedRecent = module {
                            use kitsune_p2p::gossip::sharded_gossip::*;
                            match gossip {
                                ShardedGossipWire::Initiate(Initiate { intervals, .. }) => {
                                    last_intervals = Some(intervals);
                                    // eprintln!("Initiate with: {}", agent);
                                    let kagent = agent.to_kitsune();
                                    let peer_info =
                                        peer_data.iter().find(|a| a.agent == kagent).unwrap();
                                    if let Some(alice) = &alice {
                                        let a = alice.storage_arc.interval();
                                        let b = peer_info.storage_arc.interval();
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
                                    let arc = peer_info.storage_arc.interval();
                                    let msg = HolochainP2pMockMsg::Gossip {
                                        dna: dna.clone(),
                                        module: module.clone(),
                                        gossip: ShardedGossipWire::accept(vec![arc]),
                                    };
                                    channel.send(msg.addressed((*agent).clone())).await;
                                    let this_agent_hashes =
                                        get_map(&ops_each_agent_should_hold, &agent);
                                    let window = (Timestamp::now()
                                        - std::time::Duration::from_secs(60 * 60))
                                    .unwrap()
                                        ..Timestamp::now();
                                    let this_agent_hashes: Vec<_> = this_agent_hashes
                                        .into_iter()
                                        .filter(|h| {
                                            window.contains(
                                                &get_map_ref(&data, h).header().timestamp(),
                                            )
                                        })
                                        .map(|k| get_map(&h_to_k_map, k.as_ref()))
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
                                        gossip: ShardedGossipWire::ops(filter, true),
                                    };
                                    channel.send(msg.addressed((*agent).clone())).await;
                                    if let Some(ref agent_bloom) = agent_bloom {
                                        let msg = HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: ShardedGossipWire::agents(agent_bloom.clone()),
                                        };
                                        channel.send(msg.addressed((*agent).clone())).await;
                                    }
                                }
                                ShardedGossipWire::Ops(Ops { missing_hashes, .. }) => {
                                    // eprintln!("Ops bloom with: {}", agent);
                                    let this_agent_hashes =
                                        get_map_ref(&ops_each_agent_should_hold, &agent);
                                    let num_this_agent_hashes = this_agent_hashes.len();
                                    let hashes = this_agent_hashes
                                        .iter()
                                        .map(|h| (get_map_ref(&data, h).header().timestamp(), h))
                                        .map(|(t, hash)| {
                                            (t, get_map_ref(&h_to_k_map, hash.as_ref()))
                                        });

                                    let missing_hashes = check_ops_boom(hashes, missing_hashes);
                                    let missing_hashes = match &last_intervals {
                                        Some(intervals) => missing_hashes
                                            .into_iter()
                                            .filter(|hash| {
                                                let loc = get_map(
                                                    &h_to_loc_map,
                                                    get_map_ref(&k_to_h_map, hash.as_ref()),
                                                );
                                                intervals[0].contains(loc)
                                            })
                                            .collect(),
                                        None => vec![],
                                    };
                                    gossiped_ops.extend(missing_hashes.iter().cloned());

                                    let missing_ops: Vec<_> = missing_hashes
                                        .into_iter()
                                        .map(|h| get_map_ref(&k_to_h_map, h.as_ref()))
                                        .map(|h| get_map(&data, h.as_ref()))
                                        .map(|op| {
                                            let (op, hash) = op.into_inner();
                                            (
                                                hash.into_kitsune(),
                                                holochain_p2p::WireDhtOpData { op_data: op }
                                                    .encode()
                                                    .unwrap(),
                                            )
                                        })
                                        .collect();
                                    let num_gossiped = gossiped_ops.len();
                                    let p_done = num_gossiped as f64
                                        / num_hashes_alice_should_hold as f64
                                        * 100.0;
                                    avg_gossip_freq = start_time
                                        .elapsed()
                                        .checked_div(agents_gossiped_with.len() as u32)
                                        .unwrap_or_default();
                                    let avg_gossip_size = num_gossiped / agents_gossiped_with.len();
                                    let time_to_completion = num_hashes_alice_should_hold
                                        .checked_sub(num_gossiped)
                                        .and_then(|n| n.checked_div(avg_gossip_size))
                                        .unwrap_or_default()
                                        as u32
                                        * avg_gossip_freq;
                                    let (overlap, max_could_get) = alice
                                        .as_ref()
                                        .map(|alice| {
                                            let arc = get_map(&h_to_arc_map, &agent);
                                            let a = alice.storage_arc.interval();
                                            let b = arc.interval();
                                            let num_should_hold = this_agent_hashes
                                                .iter()
                                                .filter(|hash| {
                                                    let loc = get_map(&h_to_loc_map, hash.as_ref());
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
                                        gossip: ShardedGossipWire::missing_ops(missing_ops, true),
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
                                        peer_data.len(),
                                        ops.len(),
                                        num_missed_gossips
                                    );
                                }
                                ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                                    // eprintln!(
                                    //     "Missing {} agents coverage {} with: {}",
                                    //     agents.len(),
                                    //     agents[0].storage_arc.coverage(),
                                    //     agent
                                    // );
                                    dbg!(agents.len());
                                    alice = Some(agents[0].clone());
                                }
                                _ => (),
                            }
                        }
                    }
                }
                if ops_to_hold_tx.is_some() {
                    if let Some(alice) = &alice {
                        if (alice.storage_arc.coverage() - coverage).abs() < 0.01 {
                            let hashes_that_should_be_held = data
                                .iter()
                                .filter_map(|(hash, op)| {
                                    let loc = op.dht_basis().get_loc();
                                    alice.storage_arc.contains(loc).then(|| (loc, hash.clone()))
                                })
                                .collect::<Vec<_>>();
                            let agents_that_should_be_initiated_with = h_to_k_map_agent
                                .keys()
                                .filter(|h| alice.storage_arc.contains(h.get_loc()))
                                .cloned()
                                .collect::<HashSet<_>>();
                            eprintln!("Alice covers {} and the target coverage is {}. She should hold {} out of {} ops. She should gossip with {} agents", alice.storage_arc.coverage(), coverage, hashes_that_should_be_held.len(), data.len(), agents_that_should_be_initiated_with.len());
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

    conductor.add_agent_infos(peer_data).await.unwrap();
    let apps = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = apps.into_tuple();
    let ExpectedData {
        hashes_to_be_held,
        agents_that_should_be_initiated_with,
    } = ops_to_hold_rx.await.unwrap();

    // wait_for_integration(
    //     alice.env(),
    //     hashes_to_be_held.len(),
    //     120,
    //     std::time::Duration::from_millis(500),
    // )
    // .await;
    // holochain_state::prelude::dump_tmp(alice.env());
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

    // // Call the "create" zome fn on Alice's app
    // let hash: HeaderHash = conductor.call(&alice.zome("zome1"), "create", ()).await;

    // tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
    // let all_cells = vec![&alice, &bobbo];

    // // Wait long enough for Bob to receive gossip
    // consistency_10s(&all_cells).await;
    // // let p2p = conductors[0].envs().p2p().lock().values().next().cloned().unwrap();
    // // holochain_state::prelude::dump_tmp(&p2p);
    // // holochain_state::prelude::dump_tmp(&alice.env());
    // // Verify that bobbo can run "read" on his cell and get alice's Header
    // let element: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;
    // let element = element.expect("Element was None: bobbo couldn't `get` it");

    // // Assert that the Element bobbo sees matches what alice committed
    // assert_eq!(element.header().author(), alice.agent_pubkey());
    // assert_eq!(
    //     *element.entry(),
    //     ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    // );
}

#[derive(SerializedBytes, serde::Serialize, serde::Deserialize, Debug)]
struct MockNetworkData {
    ops: HashMap<AgentPubKey, Vec<DhtOpHashed>>,
    peer_data: Vec<AgentInfoSigned>,
    uuid: String,
}

async fn generate_test_data(num_agents: usize, min_num_ops_held: usize) -> MockNetworkData {
    let uuid = nanoid::nanoid!();
    let dna_file = data_zome(uuid.clone()).await;
    let data = create_test_data(num_agents, min_num_ops_held, dna_file, uuid).await;
    let r = SerializedBytes::try_from(&data).unwrap();
    let p = std::env::temp_dir().join("mock_test_data");
    std::fs::create_dir(&p).ok();
    let p = p.join("mock_test_data");
    std::fs::write(&p, r.bytes()).unwrap();
    data
}

async fn create_test_data(
    num_agents: usize,
    approx_num_ops_held: usize,
    dna_file: DnaFile,
    uuid: String,
) -> MockNetworkData {
    let coverage = ((50.0 / num_agents as f64) * 2.0).clamp(0.0, 1.0);
    let num_storage_buckets = (1.0 / coverage).round() as u32;
    let bucket_size = u32::MAX / num_storage_buckets;
    let buckets = (0..num_storage_buckets)
        .map(|i| ArcInterval::new(i * bucket_size, i * bucket_size + bucket_size))
        .collect::<Vec<_>>();
    let mut bucket_counts = vec![0; buckets.len()];
    let mut entries = Vec::with_capacity(buckets.len() * approx_num_ops_held);
    let rng = rand::thread_rng();
    let mut rand_entry = rng.sample_iter(&Standard);
    let rand_entry = rand_entry.by_ref();
    let start = std::time::Instant::now();
    loop {
        let d: Vec<u8> = rand_entry.take(10).collect();
        let d = UnsafeBytes::from(d);
        let entry = Entry::app(d.try_into().unwrap()).unwrap();
        let hash = EntryHash::with_data_sync(&entry);
        let loc = hash.get_loc();
        if let Some(index) = buckets.iter().position(|b| b.contains(&loc)) {
            if bucket_counts[index] < approx_num_ops_held * 100 {
                entries.push(entry);
                bucket_counts[index] += 1;
            }
        }
        if bucket_counts
            .iter()
            .all(|&c| c >= approx_num_ops_held * 100)
        {
            break;
        }
    }
    dbg!(bucket_counts);
    dbg!(start.elapsed());

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "none".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.db_sync_level = DbSyncLevel::Off;
    config.network = Some(network);
    let mut conductor = SweetConductor::from_config(config).await;
    conductor.set_skip_publish(true);
    let mut agents = Vec::new();
    dbg!("generating agents");
    for i in 0..num_agents {
        eprintln!("generating agent {}", i);
        let agent = conductor
            .keystore()
            .clone()
            .generate_sign_keypair_from_pure_entropy()
            .await
            .unwrap();
        agents.push(agent);
    }

    dbg!("Installing apps");

    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_file.clone()])
        .await
        .unwrap();

    let cells = apps.cells_flattened();
    let mut entries = entries.into_iter();
    let entries = entries.by_ref();
    for (i, cell) in cells.iter().enumerate() {
        eprintln!("Calling {}", i);
        let e = entries.take(approx_num_ops_held).collect::<Vec<_>>();
        let _: () = conductor.call(&cell.zome("zome1"), "create_many", e).await;
    }
    let mut data = HashMap::new();
    for (i, cell) in cells.iter().enumerate() {
        eprintln!("Extracting data {}", i);
        let env = cell.env().clone();
        let ops: Vec<DhtOpHashed> = fresh_reader_test(env, |txn| {
            txn.prepare(
                "
                SELECT DhtOp.hash, DhtOp.type AS dht_type,
                Header.blob AS header_blob, Entry.blob AS entry_blob
                FROM DHtOp
                JOIN Header ON DhtOp.header_hash = Header.hash
                LEFT JOIN Entry ON Header.entry_hash = Entry.hash
            ",
            )
            .unwrap()
            .query_map([], |row| {
                let header = from_blob::<SignedHeader>(row.get("header_blob")?).unwrap();
                let op_type: DhtOpType = row.get("dht_type")?;
                let hash: DhtOpHash = row.get("hash")?;
                // Check the entry isn't private before gossiping it.
                let e: Option<Vec<u8>> = row.get("entry_blob")?;
                let entry = match e {
                    Some(entry) => Some(from_blob::<Entry>(entry).unwrap()),
                    None => None,
                };
                let op = DhtOp::from_type(op_type, header, entry).unwrap();
                let op = DhtOpHashed::with_pre_hashed(op, hash);
                Ok(op)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        });
        data.insert(cell.agent_pubkey().clone(), ops);
    }
    dbg!("Getting agent info");
    let peer_data = conductor.get_agent_infos(None).await.unwrap();
    dbg!("Done");
    MockNetworkData {
        ops: data,
        peer_data,
        uuid,
    }
}

async fn reset_peer_data(peers: Vec<AgentInfoSigned>, dna_hash: &DnaHash) -> Vec<AgentInfoSigned> {
    let coverage = ((50.0 / peers.len() as f64) * 2.0).clamp(0.0, 1.0);
    let space_hash = dna_hash.to_kitsune();
    let mut peer_data = Vec::with_capacity(peers.len());
    let rng = rand::thread_rng();
    let mut rand_string = rng.sample_iter(&Alphanumeric);
    let rand_string = rand_string.by_ref();
    for peer in peers {
        let rand_string: String = rand_string.take(16).map(char::from).collect();
        let info = AgentInfoSigned::sign(
            space_hash.clone(),
            peer.agent.clone(),
            ((u32::MAX / 2) as f64 * coverage) as u32,
            vec![url2::url2!(
                "kitsune-proxy://CIW6PxKxs{}MSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-",
                rand_string
            )
            .into()],
            peer.signed_at_ms,
            (std::time::UNIX_EPOCH.elapsed().unwrap() + std::time::Duration::from_secs(60_000_000))
                .as_millis() as u64,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap();
        peer_data.push(info);
    }
    peer_data
}

async fn data_zome(uuid: String) -> DnaFile {
    let entry_def = EntryDef::default_with_id("entrydef");

    let zome = InlineZome::new(uuid.clone(), vec![entry_def.clone()])
        .callback("create_many", move |api, entries: Vec<Entry>| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            for entry in entries {
                api.create(CreateInput::new(
                    entry_def_id.clone(),
                    entry,
                    ChainTopOrdering::default(),
                ))?;
            }
            Ok(())
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        });
    let (dna_file, _) = SweetDnaFile::from_inline_zome(uuid, "zome1", zome)
        .await
        .unwrap();
    dna_file
}

fn get_map<Q: ?Sized, K, V>(data: &HashMap<K, V>, key: &Q) -> V
where
    K: std::borrow::Borrow<Q>,
    V: Clone,
    Q: std::hash::Hash + Eq,
    K: std::hash::Hash + Eq,
{
    data.get(key).unwrap().clone()
}

fn get_map_ref<'a, Q: ?Sized, K, V>(data: &'a HashMap<K, V>, key: &Q) -> &'a V
where
    K: std::borrow::Borrow<Q>,
    V: Clone,
    Q: std::hash::Hash + Eq,
    K: std::hash::Hash + Eq,
{
    data.get(key).unwrap()
}
