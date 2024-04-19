use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::*;
use holochain_conductor_api::conductor::ConductorTuningParams;
use holochain_sqlite::db::{DbKindT, DbWrite};
use holochain_sqlite::prelude::DatabaseResult;
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

    use holochain::test_utils::inline_zomes::simple_create_read_zome;
    use kitsune_p2p_types::config::KitsuneP2pConfig;

    let _g = holochain_trace::test_run();
    const NUM_CONDUCTORS: usize = 3;

    let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

    let mut tuning =
        kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
    tuning.gossip_strategy = "none".to_string();

    let mut network = KitsuneP2pConfig::from_signal_addr(signal_url);
    network.tuning_params = Arc::new(tuning);
    let mut config = ConductorConfig::empty();
    config.network = network;
    config.tuning_params = Some(ConductorTuningParams {
        sys_validation_retry_delay: Some(std::time::Duration::from_millis(100)),
        ..Default::default()
    });
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency(10, [&alice, &bobbo, &carol])
        .await
        .unwrap();

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
async fn multi_conductor() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    holochain_trace::test_run();

    const NUM_CONDUCTORS: usize = 3;

    let config = SweetConductorConfig::rendezvous(true).tune_conductor(|config| {
        // The default is 10s which makes the test very slow in the case that get requests in the sys validation workflow
        // hit a conductor which isn't serving that data yet. Speed up by retrying more quickly.
        config.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
    });

    let mut conductors = SweetConductorBatch::from_config_rendezvous(NUM_CONDUCTORS, config).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency(10, [&alice, &bobbo, &carol])
        .await
        .unwrap();

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
    tracing::info!(target: "TEST", "@!@! - metrics: {metrics}");

    // See if we can fetch network stats from bobbo
    let stats = conductors[1].dump_network_stats().await?;
    tracing::info!(target: "TEST", "@!@! - stats: {stats}");

    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn sharded_consistency() {
    use holochain::test_utils::{
        consistency::local_machine_session, inline_zomes::simple_create_read_zome,
    };

    let _g = holochain_trace::test_run();
    const NUM_CONDUCTORS: usize = 3;
    const NUM_CELLS: usize = 5;

    let config = SweetConductorConfig::standard().tune(|tuning| {
        tuning.gossip_strategy = "sharded-gossip".to_string();
        tuning.gossip_dynamic_arcs = true;
    });
    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;
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

    let conductor_handles: Vec<_> = conductors.iter().map(|c| c.raw_handle()).collect();
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
    use holochain_types::inline_zome::InlineZomeSet;

    let _g = holochain_trace::test_run();
    let mut entry_def = EntryDef::default_from_id("entrydef");
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

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zome.0).await;
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    conductors.exchange_peer_info().await;

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    await_consistency(60, [&alice, &bobbo]).await.unwrap();

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
    await_consistency(60, [&alice, &bobbo]).await.unwrap();

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

    check_for_private_entries(alice.dht_db().clone()).await;
    check_for_private_entries(conductors[0].get_cache_db(alice.cell_id()).await.unwrap()).await;
    check_for_private_entries(bobbo.dht_db().clone()).await;
    check_for_private_entries(conductors[1].get_cache_db(bobbo.cell_id()).await.unwrap()).await;
}

#[tracing::instrument(skip_all)]
async fn check_for_private_entries<Kind: DbKindT>(env: DbWrite<Kind>) {
    let count: usize = env.read_async(move |txn| -> DatabaseResult<usize> {
        Ok(txn.query_row(
            "select count(action.rowid) from action join entry on action.entry_hash = entry.hash where private_entry = 1",
            [],
            |row| row.get(0),
        )?)
    }).await.unwrap();
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
    let records = records.into_iter().flatten().collect();
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
