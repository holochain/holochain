use hdk::prelude::*;
use holochain::sweettest::SweetConductorConfig;
use holochain::sweettest::*;
use holochain_state::dht_store::DhtStore;
use unwrap_to::unwrap_to;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

/// Test that op publishing is sufficient for bobbo to get alice's op
/// even with gossip disabled.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn publish() {
    use holochain::{retry_until_timeout, test_utils::inline_zomes::simple_create_read_zome};

    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true)
        .tune_network_config(|nc| {
            nc.disable_gossip = true;
        })
        .tune_conductor(|tune| {
            // Publishing an op is a best-effort notification. Keep the
            // publish loop and the per-op publish cooldown short so that a
            // missed notification is retried within this test's timeout.
            tune.publish_trigger_interval = Some(std::time::Duration::from_millis(500));
            tune.min_publish_interval = Some(std::time::Duration::from_millis(500));
        });

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

/// Ops authored without storage peers stay eligible when a peer is discovered.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn publish_recovers_after_storage_peer_discovery() {
    use holochain::core::queue_consumer::{TriggerSender, WorkComplete};
    use holochain::core::workflow::publish_dht_ops_workflow::publish_dht_ops_workflow;
    use holochain::test_utils::{inline_zomes::simple_create_read_zome, retry_fn_until_timeout};
    use holochain_p2p::HolochainP2pDna;
    use holochain_state::dht_store::GetAgentActivityOptions;
    use holochain_types::activity::ChainItems;
    use std::{sync::Arc, time::Duration};

    let config = SweetConductorConfig::rendezvous(false).tune_network_config(|nc| {
        nc.disable_gossip = true;
    });
    let min_publish_interval = config.conductor_tuning_params().min_publish_interval();
    let mut author = SweetConductor::from_config_rendezvous(
        config
            .clone()
            .tune_network_config(|nc| nc.target_arc_factor = 0),
        SweetLocalRendezvous::new().await,
    )
    .await;
    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;
    let author_cell = author
        .setup_app("app", [&dna])
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    let hash: ActionHash = author.call(&author_cell.zome("simple"), "create", ()).await;
    let author_store = author_cell.dht_store().as_read();
    let activity = author_store
        .get_agent_activity(
            author_cell.agent_pubkey(),
            &ChainQueryFilter::new(),
            &GetAgentActivityOptions::default(),
        )
        .await
        .unwrap();
    let ChainItems::Hashes(chain) = &activity.valid_activity else {
        panic!("expected authored activity hashes");
    };
    assert_eq!(
        chain.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );

    let before = author_store
        .get_ops_to_publish(author_cell.agent_pubkey(), min_publish_interval)
        .await
        .unwrap();
    let (trigger, receiver) =
        TriggerSender::new_with_loop(Duration::from_secs(60)..Duration::from_secs(300), true);
    let complete = publish_dht_ops_workflow(
        author_cell.dht_store().clone(),
        Arc::new(HolochainP2pDna::new(
            author.holochain_p2p().clone(),
            author_cell.dna_hash().clone(),
        )),
        trigger,
        author_cell.agent_pubkey().clone(),
        min_publish_interval,
    )
    .await
    .unwrap();
    assert_eq!(complete, WorkComplete::Complete);
    assert!(
        !receiver.is_paused(),
        "unpublished ops must retain the paced retry loop"
    );
    assert_eq!(
        author_store
            .get_ops_to_publish(author_cell.agent_pubkey(), min_publish_interval)
            .await
            .unwrap(),
        before,
        "no-destination admission must not impose the publication throttle"
    );

    let mut storage =
        SweetConductor::from_config_rendezvous(config, author.rendezvous().unwrap().clone()).await;
    let storage_cell = storage
        .setup_app("app", [&dna])
        .await
        .unwrap()
        .into_cells()
        .remove(0);
    storage
        .declare_full_storage_arcs(storage_cell.dna_hash())
        .await;
    let storage_store = storage_cell.dht_store().as_read();
    assert!(!storage_store
        .has_genesis(author_cell.agent_pubkey())
        .await
        .unwrap());
    retry_fn_until_timeout(
        || SweetConductor::exchange_peer_info([&author, &storage]),
        Some(10_000),
        Some(10),
    )
    .await
    .unwrap();
    author
        .raw_handle()
        .get_cell_triggers(author_cell.cell_id())
        .await
        .unwrap()
        .publish_dht_ops
        .trigger(&"storage peer discovered");

    retry_fn_until_timeout(
        || async {
            let received = storage_store
                .get_agent_activity(
                    author_cell.agent_pubkey(),
                    &ChainQueryFilter::new(),
                    &GetAgentActivityOptions::default(),
                )
                .await
                .unwrap();
            received.valid_activity == activity.valid_activity
                && storage_store
                    .get_record_details(&hash, None)
                    .await
                    .unwrap()
                    .is_some()
        },
        Some(20_000),
        Some(100),
    )
    .await
    .expect("genesis and later activity must publish without waiting out the throttle");
    let record = storage_store
        .retrieve_record(&hash, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.action_address(), &hash);
    assert_eq!(record.action().author(), author_cell.agent_pubkey());
    author.shutdown().await;
    storage.shutdown().await;
}

#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
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

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    // Wait long enough for Bob to receive gossip
    await_consistency([&alice, &bobbo, &carol]).await.unwrap();

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

/// Flaky on Windows separately from the pending fixes alongside Iroh networking upgrade.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
async fn private_entries_update_consistency() {
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
        .function("update", |api, hash: ActionHash| {
            let updated_entry = Entry::app(PrivateEntry {}.try_into().unwrap()).unwrap();
            api.update(UpdateInput {
                original_action_address: hash,
                entry: updated_entry,
                chain_top_ordering: ChainTopOrdering::Strict,
            })
            .map_err(Into::into)
        });

    let mut conductors = SweetConductorBatch::standard(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zome.0).await;
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();
    let ((alice,), (bobbo,)) = apps.into_tuples();

    await_consistency([&alice, &bobbo]).await.unwrap();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    await_consistency([&alice, &bobbo]).await.unwrap();

    // Call the "update" zome fn on Alice's app to update the previously created private entry
    let _: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "update", hash)
        .await;

    // Make sure that the update of the private entry reaches consistency
    await_consistency([&alice, &bobbo]).await.unwrap();
}

/// Flaky on Windows separately from the pending fixes alongside Iroh networking upgrade.
#[cfg(feature = "test_utils")]
#[tokio::test(flavor = "multi_thread")]
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

    let mut conductors = SweetConductorBatch::standard(2).await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zome.0).await;
    let dnas = vec![dna_file];

    let apps = conductors.setup_app("app", &dnas).await.unwrap();
    let ((alice,), (bobbo,)) = apps.into_tuples();

    await_consistency([&alice, &bobbo]).await.unwrap();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome(SweetInlineZomes::COORDINATOR), "create", ())
        .await;

    await_consistency([&alice, &bobbo]).await.unwrap();

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
    await_consistency([&alice, &bobbo]).await.unwrap();

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

    check_for_private_entries(alice.dht_store()).await;
    check_for_private_entries(bobbo.dht_store()).await;
}

/// Private entries are never placed in the shared public `Entry` table; they
/// live only in the separate `PrivateEntry` table. Assert the public table
/// holds no private-visibility entries.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
async fn check_for_private_entries(dht_store: &DhtStore) {
    let count = dht_store
        .as_read()
        .count_private_entries_in_public_table()
        .await
        .unwrap();
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
            assert_eq!(
                action.hashed.content.author(),
                zome.cell_id().agent_pubkey()
            );
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
