use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductor, SweetNetwork, SweetZome};
use holochain::sweettest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::host_fn_caller::Post;
use holochain::test_utils::wait_for_integration_1m;
use holochain::test_utils::wait_for_integration_with_others_10s;
use holochain::test_utils::WaitOps;
use holochain_sqlite::db::{DbKindT, DbWrite};
use holochain_state::prelude::fresh_reader_test;
use unwrap_to::unwrap_to;

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
async fn private_entries_dont_leak() {
    use holochain::test_utils::consistency_10s;

    let _g = observability::test_run().ok();
    let mut entry_def = EntryDef::default_with_id("entrydef");
    entry_def.visibility = EntryVisibility::Private;

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    struct PrivateEntry;

    let zome = InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, _: ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(PrivateEntry {}.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                entry_def_id,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("get", |api, hash: AnyDhtHash| {
            api.get(vec![GetInput::new(hash, GetOptions::default())])
                .map_err(Into::into)
        })
        .callback("get_details", |api, hash: AnyDhtHash| {
            api.get_details(vec![GetInput::new(hash, GetOptions::default())])
                .map_err(Into::into)
        });

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", zome)
        .await
        .unwrap();
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;
    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    consistency_10s(&[&alice, &bobbo]).await;

    let entry_hash =
        EntryHash::with_data_sync(&Entry::app(PrivateEntry {}.try_into().unwrap()).unwrap());

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome("zome1"),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome("zome1"),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;

    // Bobbo creates the same private entry.
    let bob_hash: HeaderHash = conductors[1].call(&bobbo.zome("zome1"), "create", ()).await;
    consistency_10s(&[&alice, &bobbo]).await;

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome("zome1"),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome("zome1"),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome("zome1"),
        bob_hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome("zome1"),
        bob_hash.clone(),
        entry_hash.clone(),
    )
    .await;

    check_for_private_entries(alice.dht_env().clone());
    check_for_private_entries(conductors[0].get_cache_env(alice.cell_id()).unwrap());
    check_for_private_entries(bobbo.dht_env().clone());
    check_for_private_entries(conductors[1].get_cache_env(bobbo.cell_id()).unwrap());
}

fn check_for_private_entries<Kind: DbKindT>(env: DbWrite<Kind>) {
    let count: usize = fresh_reader_test(env, |txn| {
        txn.query_row(
            "select count(header.rowid) from header join entry on header.entry_hash = entry.hash where private_entry = 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
    });
    assert_eq!(count, 0);
}

async fn check_all_gets_for_private_entry(
    conductor: &SweetConductor,
    zome: &SweetZome,
    header_hash: HeaderHash,
    entry_hash: EntryHash,
) {
    let mut elements: Vec<Option<Element>> = conductor
        .call(zome, "get", AnyDhtHash::from(header_hash.clone()))
        .await;
    let e: Vec<Option<Element>> = conductor
        .call(zome, "get", AnyDhtHash::from(entry_hash.clone()))
        .await;
    elements.extend(e);
    let details: Vec<Option<Details>> = conductor
        .call(zome, "get_details", AnyDhtHash::from(header_hash.clone()))
        .await;
    elements.extend(
        details
            .into_iter()
            .map(|d| d.map(|d| unwrap_to!(d => Details::Element).clone().element)),
    );
    let elements = elements.into_iter().filter_map(|a| a).collect();
    check_elements_for_private_entry(zome.cell_id().agent_pubkey().clone(), elements);
    let entries: Vec<Option<Details>> = conductor
        .call(zome, "get_details", AnyDhtHash::from(entry_hash.clone()))
        .await;
    for entry in entries {
        let entry = match entry {
            Some(e) => e,
            None => continue,
        };
        let details = unwrap_to!(entry=> Details::Entry).clone();
        let headers = details.headers;
        for header in headers {
            assert_eq!(header.header().author(), zome.cell_id().agent_pubkey());
        }
    }
}

fn check_elements_for_private_entry(caller: AgentPubKey, elements: Vec<Element>) {
    for element in elements {
        if *element.header().author() == caller {
            assert_ne!(*element.entry(), ElementEntry::Hidden);
        } else {
            assert_eq!(*element.entry(), ElementEntry::Hidden);
        }
    }
}
