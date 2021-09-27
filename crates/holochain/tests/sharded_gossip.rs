use std::collections::HashMap;
use std::sync::Arc;

use ::fixt::prelude::*;
use hdk::prelude::*;
use holo_hash::{DhtOpHash, DnaHash};
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductor, SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::consistency_10s;
use holochain::test_utils::inline_zomes::simple_create_read_zome;
use holochain_keystore::KeystoreSenderExt;
use holochain_p2p::dht_arc::{ArcInterval, DhtArc};
use holochain_p2p::DnaHashExt;
use holochain_p2p::HolochainP2pSender;
use holochain_state::prelude::from_blob;
use holochain_state::test_utils::fresh_reader_test;
use holochain_types::dht_op::{DhtOp, DhtOpHashed, DhtOpType};
use holochain_types::prelude::DnaFile;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::fixt::*;
use kitsune_p2p::KitsuneP2pConfig;
use rand::distributions::uniform::{UniformDuration, UniformSampler};
use rand::distributions::Alphanumeric;
use rand::distributions::{Standard, Uniform};
use rand::{thread_rng, Rng};

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
async fn mock_network_sharded_gossip() {
    use holochain::conductor::ConductorBuilder;
    use holochain_p2p::{
        dht_arc::{DhtArcBucket, DhtArcSet},
        mock_network::{
            AddressedHolochainP2pMockMsg, HolochainP2pMockChannel, HolochainP2pMockMsg,
        },
        AgentPubKeyExt,
    };
    use kitsune_p2p::{
        event::full_time_range, gossip::sharded_gossip::test_utils::create_agent_bloom,
        KitsuneBinType,
    };

    let _g = observability::test_run().ok();
    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    // vec![url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-").into()],
    // let mut peer_data = Vec::new();
    // let mut agent_iter =
    //     holo_hash::fixt::AgentPubKeyFixturator::new(Unpredictable).map(|a| a.to_kitsune());
    // for _ in 0..150 {
    //     let rand_string: String = thread_rng()
    //         .sample_iter(&Alphanumeric)
    //         .take(16)
    //         .map(char::from)
    //         .collect();
    //     let info = AgentInfoSigned::sign(
    //         space_hash.clone(),
    //         // Arc::new(agent_iter.next().unwrap()),
    //         agent_iter.next().unwrap(),
    //         ((u32::MAX / 2) as f64 * 0.66) as u32,
    //         vec![url2::url2!(
    //             "kitsune-proxy://CIW6PxKxs{}MSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-",
    //             rand_string
    //         )
    //         .into()],
    //         0,
    //         std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
    //         |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
    //     )
    //     .await
    //     .unwrap();
    //     peer_data.push(info);
    // }
    let p = std::env::temp_dir()
        .join("mock_test_data")
        .join("mock_test_data");
    let data = std::fs::read(p).unwrap();
    let data = SerializedBytes::from(UnsafeBytes::from(data))
        .try_into()
        .unwrap();

    let MockNetworkData { peer_data, ops } = data;
    let peer_data = reset_peer_data(peer_data, dna_file.dna_hash()).await;
    let mut arcs = Vec::new();
    for peer in &peer_data {
        arcs.push(peer.storage_arc);
        println!(
            "{}:{}",
            peer.storage_arc.interval().to_ascii(10),
            peer.agent.get_loc()
        );
    }
    let bucket = DhtArcBucket::new(DhtArc::full(0), arcs.clone());
    println!("{}\n{:?}", bucket, bucket.density());
    let bucket = DhtArcBucket::new(DhtArc::with_coverage(0, 0.1), arcs.clone());
    println!("{:?}", bucket.density());
    let bucket = DhtArcBucket::new(DhtArc::with_coverage(u32::MAX / 2, 0.1), arcs.clone());
    println!("{:?}", bucket.density());

    let agent_bloom = create_agent_bloom(peer_data.iter(), None);
    let (from_kitsune_tx, to_kitsune_rx, mut channel) =
        HolochainP2pMockChannel::channel(peer_data.clone(), 1000);
    tokio::task::spawn({
        let peer_data = peer_data.clone();
        async move {
            let mut alice: Option<Arc<AgentInfoSigned>> = None;
            let mut agents_gossiped_with = HashSet::new();
            let mut num_missed_gossips = 0;
            while let Some((msg, respond)) = channel.next().await {
                let AddressedHolochainP2pMockMsg { agent, msg } = msg;
                match msg {
                    HolochainP2pMockMsg::Wire {
                        to_agent,
                        from_agent,
                        dna,
                        msg,
                    } => (),
                    HolochainP2pMockMsg::CallResp(_) => (),
                    HolochainP2pMockMsg::PeerGet(_) => (),
                    HolochainP2pMockMsg::PeerGetResp(_) => (),
                    HolochainP2pMockMsg::PeerQuery(_) => (),
                    HolochainP2pMockMsg::PeerQueryResp(_) => (),
                    HolochainP2pMockMsg::Gossip {
                        dna,
                        module,
                        gossip,
                    } => {
                        if let kitsune_p2p::GossipModuleType::ShardedRecent = module {
                            use kitsune_p2p::gossip::sharded_gossip::*;
                            match gossip {
                                ShardedGossipWire::Initiate { .. } => {
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
                                    let msg = HolochainP2pMockMsg::Gossip {
                                        dna: dna.clone(),
                                        module: module.clone(),
                                        gossip: ShardedGossipWire::accept(vec![ArcInterval::Full]),
                                    };
                                    channel.send(msg.addressed(agent.clone())).await;
                                    let msg = HolochainP2pMockMsg::Gossip {
                                        dna: dna.clone(),
                                        module: module.clone(),
                                        gossip: ShardedGossipWire::ops(
                                            EncodedTimedBloomFilter::MissingAllHashes {
                                                time_window: full_time_range(),
                                            },
                                            true,
                                        ),
                                    };
                                    channel.send(msg.addressed(agent.clone())).await;
                                    let msg = HolochainP2pMockMsg::Gossip {
                                        dna: dna.clone(),
                                        module: module.clone(),
                                        gossip: ShardedGossipWire::agents(agent_bloom.clone()),
                                    };
                                    channel.send(msg.addressed(agent.clone())).await;
                                }
                                ShardedGossipWire::Ops { .. } => {
                                    // eprintln!("Ops bloom with: {}", agent);
                                    let msg = HolochainP2pMockMsg::Gossip {
                                        dna,
                                        module,
                                        gossip: ShardedGossipWire::missing_ops(vec![], true),
                                    };
                                    channel.send(msg.addressed(agent.clone())).await;
                                }
                                ShardedGossipWire::MissingOps { .. } => {
                                    // eprintln!("Missing ops with: {}", agent);
                                }
                                ShardedGossipWire::MissingAgents(MissingAgents { agents }) => {
                                    eprintln!(
                                        "Missing {} agents coverage {} with: {}",
                                        agents.len(),
                                        agents[0].storage_arc.coverage(),
                                        agent
                                    );
                                    alice = Some(agents[0].clone());
                                }
                                _ => (),
                            }
                            eprintln!(
                                "Gossiped with {} out of {} and gossiped with {} nodes outside of arc",
                                agents_gossiped_with.len(),
                                peer_data.len(),
                                num_missed_gossips
                            );
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
    let apps = conductor.setup_app("app", &[dna_file]).await.unwrap();

    let (alice,) = apps.into_tuple();

    // // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductor.call(&alice.zome("zome1"), "create", ()).await;

    tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
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
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn generate_test_data() {
    let r = create_test_data(1000, 10).await;
    let r = SerializedBytes::try_from(r).unwrap();
    let p = std::env::temp_dir().join("mock_test_data");
    std::fs::create_dir(&p).ok();
    let p = p.join("mock_test_data");
    std::fs::write(&p, r.bytes()).unwrap();
}

async fn create_test_data(num_agents: usize, approx_num_ops_held: usize) -> MockNetworkData {
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

    let entry_def = EntryDef::default_with_id("entrydef");

    let zome = InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, entries: Vec<Entry>| {
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
    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "none".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);
    let mut conductor = SweetConductor::from_config(config).await;
    conductor.set_skip_publish(true);
    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", zome)
        .await
        .unwrap();
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
        .setup_app_for_agents("app", &agents, &[dna_file])
        .await
        .unwrap();

    let cells = apps.cells_flattened();
    let mut entries = entries.into_iter();
    let entries = entries.by_ref();
    for (i, cell) in cells.iter().enumerate() {
        eprintln!("Calling {}", i);
        let e = entries.take(approx_num_ops_held).collect::<Vec<_>>();
        let _: () = conductor.call(&cell.zome("zome1"), "create", e).await;
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
                let mut entry: Option<Entry> = None;
                let e: Option<Vec<u8>> = row.get("entry_blob")?;
                entry = match e {
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
            peer.expires_at_ms + 60_000_000,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap();
        peer_data.push(info);
    }
    peer_data
}
