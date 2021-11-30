use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::SweetNetwork;
use holochain::sweettest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::host_fn_caller::Post;
use holochain::test_utils::wait_for_integration_1m;
use holochain::test_utils::wait_for_integration_with_others_10s;
use holochain::test_utils::WaitOps;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

fn invalid_cell_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, entry: Post| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(entry.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                entry_def_id,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
}

/// Test that op publishing is sufficient for bobbo to get alice's op
/// even with gossip disabled.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn test_publish() -> anyhow::Result<()> {
    use std::sync::Arc;

    use holochain::test_utils::{consistency_10s, inline_zomes::simple_create_read_zome};
    use kitsune_p2p::KitsuneP2pConfig;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "none".to_string();

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::default();
    config.network = Some(network);
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    // Wait long enough for Bob to receive gossip
    consistency_10s(&[&alice, &bobbo, &carol]).await;

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
async fn multi_conductor() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (_carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    // Wait long enough for Bob to receive gossip
    wait_for_integration_1m(
        bobbo.dht_env(),
        WaitOps::start() * 1 + WaitOps::cold_start() * 2 + WaitOps::ENTRY * 1,
    )
    .await;

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

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
#[ignore = "I'm not convinced this test is actually adding value and worth fixing right now"]
async fn invalid_cell() -> anyhow::Result<()> {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let network = SweetNetwork::env_var_proxy().unwrap_or_else(|| {
        info!("KIT_PROXY not set using local quic network");
        SweetNetwork::local_quic()
    });
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", invalid_cell_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();
    let alice_env = alice.dht_env();
    let bob_env = bobbo.dht_env();
    let carol_env = carol.dht_env();
    let envs = vec![alice_env, bob_env, carol_env];

    conductors[1].shutdown().await;

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0]
        .call(&alice.zome("zome1"), "create", Post("1".to_string()))
        .await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(Post("1".to_string()).try_into().unwrap()).unwrap())
    );
    conductors[1].startup().await;
    let _: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;

    // Take both other conductors offline and commit a hash they don't have
    // then bring them back with the original offline.
    conductors[0].shutdown().await;
    conductors[2].shutdown().await;

    let hash: HeaderHash = conductors[1]
        .call(&bobbo.zome("zome1"), "create", Post("2".to_string()))
        .await;
    conductors[1].shutdown().await;
    conductors[0].startup().await;
    let r: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_none());
    conductors[2].startup().await;
    let r: Option<Element> = conductors[2]
        .call(&carol.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_none());
    conductors[1].startup().await;

    let _: HeaderHash = conductors[0]
        .call(&alice.zome("zome1"), "create", Post("3".to_string()))
        .await;
    let _: HeaderHash = conductors[1]
        .call(&bobbo.zome("zome1"), "create", Post("4".to_string()))
        .await;
    let _: HeaderHash = conductors[2]
        .call(&carol.zome("zome1"), "create", Post("5".to_string()))
        .await;

    let expected_count = WaitOps::start() * 3 + WaitOps::ENTRY * 5;
    wait_for_integration_with_others_10s(alice_env, &envs[..], expected_count, None).await;
    let r: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_some());
    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn sharded_consistency() {
    use std::sync::Arc;

    use holochain::test_utils::{
        consistency::local_machine_session, inline_zomes::simple_create_read_zome,
    };
    use kitsune_p2p::KitsuneP2pConfig;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;
    const NUM_CELLS: usize = 5;

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "sharded-gossip".to_string();
    tuning.gossip_dynamic_arcs = true;

    let mut network = KitsuneP2pConfig::default();
    network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    network.tuning_params = Arc::new(tuning);
    let config = ConductorConfig {
        network: Some(network),
        ..Default::default()
    };
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_create_read_zome())
        .await
        .unwrap();
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();

    let ((alice,), (bobbo,), (_carol,)) = apps.into_tuples();

    for i in 0..NUM_CELLS {
        conductors.setup_app(&i.to_string(), &dnas).await.unwrap();
    }
    conductors.exchange_peer_info().await;
    conductors.force_all_publish_dht_ops().await;
    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    let conductor_handles: Vec<_> = conductors.iter().map(|c| c.handle()).collect();
    local_machine_session(&conductor_handles, std::time::Duration::from_secs(60)).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn large_gossip() {
    use std::sync::Arc;

    use ::fixt::prelude::*;
    use holochain::conductor::handle::DevSettingsDelta;
    use holochain::test_utils::inline_zomes::simple_create_read_zome;
    use holochain::test_utils::{consistency_10s, wait_for_integration};
    use holochain_p2p::actor::HolochainP2pRefToDna;
    use holochain_p2p::HolochainP2pDnaT;
    use holochain_state::prelude::{
        dump_tmp, insert_entry, insert_header, insert_op_lite_into_authored, SourceChain,
    };
    use holochain_types::dht_op::produce_op_lights_from_iter;
    use holochain_types::prelude::DisabledAppReason;
    use kitsune_p2p::KitsuneP2pConfig;

    let _g = observability::test_run().ok();

    const NUM_CONDUCTORS: usize = 2;
    let existing_data = std::env::var_os("DB_DATA").map(|s| s.to_string_lossy().to_string());

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_historic_inbound_target_mbps = 10.0;
    tuning.gossip_historic_outbound_target_mbps = 10.0;
    tuning.tx2_channel_count_per_connection = 2;

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let config = ConductorConfig {
        network: Some(network),
        ..Default::default()
    };
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, zome) =
        SweetDnaFile::from_inline_zome("".into(), "zome1", simple_create_read_zome())
            .await
            .unwrap();

    let dna_hash = dna_file.dna_hash().clone();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    for c in conductors.iter() {
        c.update_dev_settings(DevSettingsDelta {
            publish: Some(false),
            ..Default::default()
        });
    }
    // tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Wait long enough for Bob to receive gossip
    let all_cells = vec![&alice, &bobbo];

    // Call the "create" zome fn on Alice's app
    let _hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    // Wait long enough for Bob to receive gossip
    consistency_10s(&all_cells).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    for c in conductors.iter() {
        c.disable_app("app".to_string(), DisabledAppReason::User)
            .await
            .unwrap();
    }

    match existing_data {
        Some(db) => {
            load_data(db, alice.dht_env().path().display());
        }
        None => {
            let sc = SourceChain::new(
                alice.authored_env().clone(),
                alice.dht_env().clone(),
                conductors[0].keystore(),
                alice.agent_pubkey().clone(),
            )
            .await
            .unwrap();

            let entry = Entry::App(fixt!(AppEntryBytes));
            let entry_hash = EntryHash::with_data_sync(&entry);
            let header_builder = builder::Create {
                entry_type: EntryType::App(AppEntryType::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash,
            };
            for i in 0..400_000 {
                let s = std::time::Instant::now();
                sc.put(
                    Some(zome.clone()),
                    header_builder.clone(),
                    Some(entry.clone()),
                    ChainTopOrdering::default(),
                )
                .await
                .unwrap();
                eprintln!("{} {:?}", i, s.elapsed());
            }
            let s = std::time::Instant::now();
            alice
                .dht_env()
                .async_commit(move |txn| {
                    sc.scratch()
                        .apply(|s| {
                            for entry in s.drain_entries() {
                                insert_entry(txn, entry).unwrap();
                            }
                            for (_, shh) in s.drain_zomed_headers() {
                                let s = std::time::Instant::now();
                                let entry_hash = shh.header().entry_hash().cloned();
                                let item = (shh.as_hash(), shh.header(), entry_hash);
                                let ops =
                                    produce_op_lights_from_iter(vec![item].into_iter(), 1).unwrap();
                                let timestamp = shh.header().timestamp();
                                let header = shh.header().clone();
                                dbg!(s.elapsed());
                                let s = std::time::Instant::now();
                                insert_header(txn, shh.clone()).unwrap();
                                dbg!(s.elapsed());
                                let s = std::time::Instant::now();
                                for op in ops {
                                    let s = std::time::Instant::now();
                                    let op_type = op.get_type();
                                    let op_order =
                                        holochain_types::dht_op::OpOrder::new(op_type, timestamp);
                                    let (_, op_hash) =
                                        holochain_types::dht_op::UniqueForm::op_hash(
                                            op_type,
                                            header.clone(),
                                        )
                                        .unwrap();
                                    dbg!(s.elapsed());
                                    let s = std::time::Instant::now();
                                    insert_op_lite_into_authored(
                                        txn,
                                        op,
                                        op_hash.clone(),
                                        op_order,
                                        timestamp,
                                    )
                                    .unwrap();
                                    dbg!(s.elapsed());
                                }
                                dbg!(s.elapsed());
                            }
                        })
                        .unwrap();
                    holochain_sqlite::prelude::DatabaseResult::Ok(())
                })
                .await
                .unwrap();

            dump_tmp(alice.dht_env());

            dbg!(s.elapsed());
        }
    }

    for c in conductors.iter() {
        c.enable_app("app".to_string()).await.unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    dbg!(alice.cell_id());
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    let p2p = conductors[1].holochain_p2p().to_dna(dna_hash);
    let h = hash.clone();
    let jh = tokio::spawn(async move {
        let hash: AnyDhtHash = h.into();
        let mut c = 0;
        let mut t = std::time::Duration::default();
        let mut m = std::time::Duration::default();
        loop {
            c += 1;
            let s = std::time::Instant::now();
            p2p.get(hash.clone(), Default::default()).await.unwrap();
            let el = s.elapsed();
            t += el;
            m = std::cmp::max(m, el);
            let a = t / c;
            eprintln!("Get time {:?}, avg {:?}, max {:?}", el, a, m);
        }
    });

    let b_env = bobbo.dht_env().clone();
    tokio::spawn({
        let b_env = b_env;
        async move {
            let b_env = b_env;
            loop {
                // wait_for_integration(&b_env, 1_200_000, 10000, std::time::Duration::from_secs(5)).await;
                let dump = holochain::conductor::integration_dump(&b_env.clone().into())
                    .await
                    .unwrap();
                eprintln!("{:?}", dump);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    let mut c = 0;
    let mut t = std::time::Duration::default();
    let mut m = std::time::Duration::default();
    let mut l = std::time::Instant::now();
    loop {
        c += 1;
        let s = std::time::Instant::now();
        let _: Option<Element> = conductors[1]
            .call(&bobbo.zome("zome1"), "read", hash.clone())
            .await;
        let el = s.elapsed();
        t += el;
        m = std::cmp::max(m, el);
        let a = t / c;
        if l.elapsed().as_secs() > 5 {
            eprintln!("Call time {:?}, avg {:?}, max {:?}", el, a, m);
            l = std::time::Instant::now();
        }
    }
    jh.abort();
    let _ = jh.await;
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn large_validation() {
    use std::sync::Arc;

    use holochain::test_utils::inline_zomes::simple_create_read_zome;
    use holochain::test_utils::{consistency_10s, wait_for_integration};
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_types::prelude::DisabledAppReason;
    use kitsune_p2p::KitsuneP2pConfig;

    let _g = observability::test_run().ok();

    const NUM_CONDUCTORS: usize = 1;
    let existing_data = std::env::var_os("DB_DATA").map(|s| s.to_string_lossy().to_string());

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_historic_inbound_target_mbps = 30.0;
    tuning.gossip_historic_outbound_target_mbps = 30.0;
    tuning.tx2_channel_count_per_connection = 1;

    let mut network = KitsuneP2pConfig::default();
    network.tuning_params = Arc::new(tuning);
    let config = ConductorConfig {
        network: Some(network),
        ..Default::default()
    };
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _) =
        SweetDnaFile::from_inline_zome("".into(), "zome1", simple_create_read_zome())
            .await
            .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    // for c in conductors.iter() {
    //     c.update_dev_settings(DevSettingsDelta {
    //         publish: Some(false),
    //         ..Default::default()
    //     });
    // }
    // tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    // conductors.exchange_peer_info().await;

    let ((alice,),) = apps.into_tuples();

    // Wait long enough for Bob to receive gossip
    let all_cells = vec![&alice];

    // Call the "create" zome fn on Alice's app
    let _hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;
    consistency_10s(&all_cells).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    for c in conductors.iter() {
        c.disable_app("app".to_string(), DisabledAppReason::User)
            .await
            .unwrap();
    }

    load_data(existing_data.unwrap(), alice.dht_env().path().display());

    for c in conductors.iter() {
        c.enable_app("app".to_string()).await.unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let _hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    let n = alice.dht_env().async_commit(|txn|{
        let n = txn.execute("UPDATE DhtOp SET validation_status = NULL, validation_stage = NULL, when_integrated = NULL", [])?;
        DatabaseResult::Ok(n)
    }).await.unwrap();
    dbg!(n);
    wait_for_integration(
        alice.dht_env(),
        1_200_000,
        10000,
        std::time::Duration::from_secs(5),
    )
    .await;
}

fn load_data(db: String, other_db: std::path::Display) {
    let mut conn = rusqlite::Connection::open(db).unwrap();
    let txn = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
        .unwrap();
    txn.execute(&format!("attach database '{}' as other_db", other_db), [])
        .unwrap();
    let s = std::time::Instant::now();
    txn.execute("insert into other_db.Entry select * from main.Entry", [])
        .unwrap();
    dbg!(s.elapsed());
    let s = std::time::Instant::now();
    txn.execute("insert into other_db.Header select * from main.Header", [])
        .unwrap();
    dbg!(s.elapsed());
    let s = std::time::Instant::now();
    txn.execute("insert into other_db.DhtOp select * from main.DhtOp", [])
        .unwrap();
    dbg!(s.elapsed());
    let num: usize = txn
        .query_row("select count(rowid) from other_db.DhtOp", [], |row| {
            row.get(0)
        })
        .unwrap();
    let s = std::time::Instant::now();
    txn.commit().unwrap();
    dbg!(s.elapsed());
    dbg!(num);
}
