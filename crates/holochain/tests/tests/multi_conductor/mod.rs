use hdk::prelude::*;
use std::collections::HashMap;
//use holochain::conductor::config::{ConductorConfig, DpkiConfig};
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::*;
//use holochain_conductor_api::conductor::ConductorTuningParams;
use holochain_sqlite::db::{DbKindT, DbWrite};
use holochain_sqlite::prelude::DatabaseResult;
use holochain_types::network::Kitsune2NetworkMetricsRequest;
use unwrap_to::unwrap_to;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn dpki_publish() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::standard();
    let conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    conductors.exchange_peer_info().await;

    await_consistency(10, conductors.dpki_cells().as_slice())
        .await
        .unwrap();
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn dpki_no_publish() {
    holochain_trace::test_run();

    let config =
        SweetConductorConfig::standard().tune_network_config(|nc| nc.disable_publish = true);
    let conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

    conductors.exchange_peer_info().await;

    await_consistency(10, conductors.dpki_cells().as_slice())
        .await
        .unwrap();
}

/// Test that op publishing is sufficient for bobbo to get alice's op
/// even with gossip disabled.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn test_publish() {
    use holochain::{retry_until_timeout, test_utils::inline_zomes::simple_create_read_zome};
    use holochain_conductor_api::conductor::{ConductorConfig, NetworkConfig};

    holochain_trace::test_run();

    let config = ConductorConfig {
        network: NetworkConfig {
            disable_gossip: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
    let dna_file = SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome()))
        .await
        .0;
    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Set full storage arc for both peers and then exchange peer infos.
    conductors[0]
        .declare_full_storage_arcs(alice.dna_hash())
        .await;
    conductors[1]
        .declare_full_storage_arcs(bobbo.dna_hash())
        .await;
    conductors.exchange_peer_info().await;

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Verify that bobbo can run "read" on his cell and get alice's Action
    retry_until_timeout!(10_000, 1_000, {
        let maybe_record: Option<Record> = conductors[1]
            .call(&bobbo.zome("simple"), "read", hash.clone())
            .await;
        if maybe_record.is_some() {
            break;
        }
    });
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn multi_conductor() -> anyhow::Result<()> {
    use holochain::test_utils::inline_zomes::simple_create_read_zome;

    holochain_trace::test_run();

    const NUM_CONDUCTORS: usize = 3;

    let config = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|config| {
            // The default is 10s which makes the test very slow in the case that get requests in the sys validation workflow
            // hit a conductor which isn't serving that data yet. Speed up by retrying more quickly.
            config.sys_validation_retry_delay = Some(std::time::Duration::from_millis(100));
        })
        .no_dpki_mustfix();

    let mut conductors = SweetConductorBatch::from_config_rendezvous(NUM_CONDUCTORS, config).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let dpki_cells = conductors.dpki_cells();

    await_consistency(20, dpki_cells.as_slice()).await.unwrap();

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency(20, [&alice, &bobbo, &carol])
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
    let metrics = conductors[1]
        .dump_network_metrics(Kitsune2NetworkMetricsRequest::default())
        .await?
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect::<HashMap<_, _>>();
    tracing::info!(target: "TEST", "@!@! - metrics: {}", serde_json::to_string_pretty(&metrics).unwrap());

    // See if we can fetch network stats from bobbo
    let stats = conductors[1].dump_network_stats().await?;
    tracing::info!(target: "TEST", "@!@! - stats: {}", serde_json::to_string_pretty(&stats).unwrap());

    let stats = conductors[1]
        .dump_network_stats_for_app(&"app".to_string())
        .await?;
    tracing::info!(target: "TEST", "@!@! - stats by app: {}", serde_json::to_string_pretty(&stats).unwrap());

    Ok(())
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "windows", ignore = "flaky")]
async fn private_entries_dont_leak() {
    use holochain::sweettest::SweetInlineZomes;
    use holochain_types::inline_zome::InlineZomeSet;

    holochain_trace::test_run();
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

    let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zome.0).await;
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();
    let ((alice,), (bobbo,)) = apps.into_tuples();

    conductors[0]
        .require_initial_gossip_activity_for_cell(&alice, 1, std::time::Duration::from_secs(30))
        .await
        .unwrap();

    conductors[1]
        .require_initial_gossip_activity_for_cell(&bobbo, 1, std::time::Duration::from_secs(30))
        .await
        .unwrap();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    await_consistency(10, [&alice, &bobbo]).await.unwrap();

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
    await_consistency(10, [&alice, &bobbo]).await.unwrap();

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

#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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
