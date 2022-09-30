use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::{SweetConductor, SweetZome};
use holochain::sweettest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::wait_for_integration_1m;
use holochain::test_utils::WaitOps;
use holochain_sqlite::db::{DbKindT, DbWrite};
use holochain_state::prelude::fresh_reader_test;
use unwrap_to::unwrap_to;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

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

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome()))
            .await
            .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    consistency_10s(&[&alice, &bobbo, &carol]).await;

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
async fn multi_conductor() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome()))
            .await
            .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (_carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    wait_for_integration_1m(
        bobbo.dht_db(),
        WaitOps::start() * 1 + WaitOps::cold_start() * 2 + WaitOps::ENTRY * 1,
    )
    .await;

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

    // See if we can fetch metric data from bobbo
    let metrics = conductors[1].dump_network_metrics(None).await?;
    println!("@!@! - metrics: {}", metrics);

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

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome()))
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
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    let conductor_handles: Vec<_> = conductors.iter().map(|c| c.handle()).collect();
    local_machine_session(&conductor_handles, std::time::Duration::from_secs(60)).await;

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
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn private_entries_dont_leak() {
    use holochain::sweettest::SweetInlineZomes;
    use holochain::test_utils::consistency_10s;
    use holochain_types::inline_zome::InlineZomeSet;

    let _g = observability::test_run().ok();
    let mut entry_def = EntryDef::default_with_id("entrydef");
    entry_def.visibility = EntryVisibility::Private;

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    struct PrivateEntry;

    let zome = SweetInlineZomes::new(vec![entry_def.clone()], 0)
        .function("create", move |api, _: ()| {
            let entry = Entry::app(PrivateEntry {}.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Private,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("get", |api, hash: AnyDhtHash| {
            api.get(vec![GetInput::new(hash, GetOptions::default())])
                .map_err(Into::into)
        })
        .function("get_details", |api, hash: AnyDhtHash| {
            api.get_details(vec![GetInput::new(hash, GetOptions::default())])
                .map_err(Into::into)
        });

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zome.0)
        .await
        .unwrap();
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;
    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    consistency_10s(&[&alice, &bobbo]).await;

    let entry_hash =
        EntryHash::with_data_sync(&Entry::app(PrivateEntry {}.try_into().unwrap()).unwrap());

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome(SweetInlineZomes::COORDINATOR),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome(SweetInlineZomes::COORDINATOR),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;

    // Bobbo creates the same private entry.
    let bob_hash: ActionHash = conductors[1]
        .call(&bobbo.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;
    consistency_10s(&[&alice, &bobbo]).await;

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome(SweetInlineZomes::COORDINATOR),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome(SweetInlineZomes::COORDINATOR),
        hash.clone(),
        entry_hash.clone(),
    )
    .await;

    check_all_gets_for_private_entry(
        &conductors[0],
        &alice.zome(SweetInlineZomes::COORDINATOR),
        bob_hash.clone(),
        entry_hash.clone(),
    )
    .await;
    check_all_gets_for_private_entry(
        &conductors[1],
        &bobbo.zome(SweetInlineZomes::COORDINATOR),
        bob_hash.clone(),
        entry_hash.clone(),
    )
    .await;

    check_for_private_entries(alice.dht_db().clone());
    check_for_private_entries(conductors[0].get_cache_db(alice.cell_id()).unwrap());
    check_for_private_entries(bobbo.dht_db().clone());
    check_for_private_entries(conductors[1].get_cache_db(bobbo.cell_id()).unwrap());
}

fn check_for_private_entries<Kind: DbKindT>(env: DbWrite<Kind>) {
    let count: usize = fresh_reader_test(env, |txn| {
        txn.query_row(
            "select count(action.rowid) from action join entry on action.entry_hash = entry.hash where private_entry = 1",
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
    action_hash: ActionHash,
    entry_hash: EntryHash,
) {
    let mut records: Vec<Option<Record>> = conductor
        .call(zome, "get", AnyDhtHash::from(action_hash.clone()))
        .await;
    let e: Vec<Option<Record>> = conductor
        .call(zome, "get", AnyDhtHash::from(entry_hash.clone()))
        .await;
    records.extend(e);
    let details: Vec<Option<Details>> = conductor
        .call(zome, "get_details", AnyDhtHash::from(action_hash.clone()))
        .await;
    records.extend(
        details
            .into_iter()
            .map(|d| d.map(|d| unwrap_to!(d => Details::Record).clone().record)),
    );
    let records = records.into_iter().filter_map(|a| a).collect();
    check_records_for_private_entry(zome.cell_id().agent_pubkey().clone(), records);
    let entries: Vec<Option<Details>> = conductor
        .call(zome, "get_details", AnyDhtHash::from(entry_hash.clone()))
        .await;
    for entry in entries {
        let entry = match entry {
            Some(e) => e,
            None => continue,
        };
        let details = unwrap_to!(entry=> Details::Entry).clone();
        let actions = details.actions;
        for action in actions {
            assert_eq!(action.action().author(), zome.cell_id().agent_pubkey());
        }
    }
}

fn check_records_for_private_entry(caller: AgentPubKey, records: Vec<Record>) {
    for record in records {
        if *record.action().author() == caller {
            assert_ne!(*record.entry(), RecordEntry::Hidden);
        } else {
            assert_eq!(*record.entry(), RecordEntry::Hidden);
        }
    }
}
