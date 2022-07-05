#![cfg(feature = "test_utils")]

use ::fixt::prelude::*;
use hdk::prelude::*;
use holochain::{
    conductor::api::error::ConductorApiResult,
    sweettest::{SweetAgents, SweetConductor, SweetDnaFile, SweetEasyInline},
};
use holochain::{
    conductor::{api::error::ConductorApiError, CellError},
    core::workflow::error::WorkflowError,
    test_utils::WaitOps,
};
use holochain::{
    core::ribosome::guest_callback::validate::ValidateResult, test_utils::wait_for_integration_1m,
};
use holochain::{core::SourceChainError, test_utils::display_agent_infos};
use holochain_keystore::MetaLairClient;
use holochain_state::prelude::{fresh_reader_test, StateMutationError, Store, Txn};
use holochain_types::{inline_zome::InlineZomeSet, prelude::*};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{op::Op, record::RecordEntry};
use matches::assert_matches;
use tokio_stream::StreamExt;

#[derive(
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    derive_more::From,
)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

impl AppString {
    fn new<S: Into<String>>(s: S) -> Self {
        AppString(s.into())
    }
}

/// An InlineZome with simple Create and Read operations
fn simple_crud_zome() -> InlineZomeSet {
    let string_entry_def = EntryDef::default_with_id("string");
    let unit_entry_def = EntryDef::default_with_id("unit");

    SweetEasyInline::new(vec![string_entry_def.clone(), unit_entry_def.clone()], 0)
        .callback("create_string", move |api, s: AppString| {
            let entry = Entry::app(AppString::from(s).try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("create_unit", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(1)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("delete", move |api, action_hash: ActionHash| {
            let hash = api.delete(DeleteInput::new(action_hash, ChainTopOrdering::default()))?;
            Ok(hash)
        })
        .callback("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
        .callback("read_entry", |api, hash: EntryHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
        .callback("emit_signal", |api, ()| {
            api.emit_signal(AppSignal::new(ExternIO::encode(()).unwrap()))
                .map_err(Into::into)
        })
        .0
}

/// Simple scenario involving two agents using the same DNA
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn inline_zome_2_agents_1_dna() -> anyhow::Result<()> {
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Get two agents
    let (alice, bobbo) = SweetAgents::two(conductor.keystore()).await;

    // Install DNA and install and enable apps in conductor
    let apps = conductor
        .setup_app_for_agents("app", &[alice.clone(), bobbo.clone()], &[dna_file])
        .await
        .unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor
        .call(&alice.zome(SweetEasyInline::COORDINATOR), "create_unit", ())
        .await;

    // Wait long enough for Bob to receive gossip
    wait_for_integration_1m(
        bobbo.dht_db(),
        WaitOps::start() + WaitOps::cold_start() + WaitOps::ENTRY,
    )
    .await;

    // Verify that bobbo can run "read" on his cell and get alice's Action
    let records: Vec<Option<Record>> = conductor
        .call(&bobbo.zome(SweetEasyInline::COORDINATOR), "read", hash)
        .await;
    let record = records
        .into_iter()
        .next()
        .unwrap()
        .expect("Record was None: bobbo couldn't `get` it");

    // Assert that the Record bobbo sees matches what alice committed
    assert_eq!(record.action().author(), alice.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

/// Simple scenario involving three agents using an app with two DNAs
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn inline_zome_3_agents_2_dnas() -> anyhow::Result<()> {
    observability::test_run().ok();
    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_foo, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;
    let (dna_bar, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;

    let agents = SweetAgents::get(conductor.keystore(), 3).await;

    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_foo, dna_bar])
        .await
        .unwrap();

    let ((alice_foo, alice_bar), (bobbo_foo, bobbo_bar), (_carol_foo, carol_bar)) =
        apps.into_tuples();

    assert_eq!(alice_foo.agent_pubkey(), alice_bar.agent_pubkey());
    assert_eq!(bobbo_foo.agent_pubkey(), bobbo_bar.agent_pubkey());
    assert_ne!(alice_foo.agent_pubkey(), bobbo_foo.agent_pubkey());

    //////////////////////
    // END SETUP

    let hash_foo: ActionHash = conductor
        .call(
            &alice_foo.zome(SweetEasyInline::COORDINATOR),
            "create_unit",
            (),
        )
        .await;
    let hash_bar: ActionHash = conductor
        .call(
            &alice_bar.zome(SweetEasyInline::COORDINATOR),
            "create_unit",
            (),
        )
        .await;

    // Two different DNAs, so ActionHashes should be different.
    assert_ne!(hash_foo, hash_bar);

    // Wait long enough for others to receive gossip
    for env in [bobbo_foo.dht_db(), carol_bar.dht_db()].iter() {
        wait_for_integration_1m(
            *env,
            WaitOps::start() * 1 + WaitOps::cold_start() * 2 + WaitOps::ENTRY * 1,
        )
        .await;
    }

    // Verify that bobbo can run "read" on his cell and get alice's Action
    // on the "foo" DNA
    let records: Vec<Option<Record>> = conductor
        .call(
            &bobbo_foo.zome(SweetEasyInline::COORDINATOR),
            "read",
            hash_foo,
        )
        .await;
    let record = records
        .into_iter()
        .next()
        .unwrap()
        .expect("Record was None: bobbo couldn't `get` it");
    assert_eq!(record.action().author(), alice_foo.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    // Verify that carol can run "read" on her cell and get alice's Action
    // on the "bar" DNA
    // Let's do it with the SweetZome instead of the SweetCell too, for fun
    let records: Vec<Option<Record>> = conductor
        .call(
            &carol_bar.zome(SweetEasyInline::COORDINATOR),
            "read",
            hash_bar,
        )
        .await;
    let record = records
        .into_iter()
        .next()
        .unwrap()
        .expect("Record was None: carol couldn't `get` it");
    assert_eq!(record.action().author(), alice_bar.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
// I can't remember what this test was for? Should we just delete?
#[ignore = "Needs to be completed when HolochainP2pEvents is accessible"]
async fn invalid_cell() -> anyhow::Result<()> {
    observability::test_run().ok();
    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_foo, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;
    let (dna_bar, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;

    // let agents = SweetAgents::get(conductor.keystore(), 2).await;

    let _app_foo = conductor.setup_app("foo", &[dna_foo]).await;

    let _app_bar = conductor.setup_app("bar", &[dna_bar]).await;

    // Give small amount of time for cells to join the network
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    tracing::debug!(dnas = ?conductor.list_dnas());
    tracing::debug!(cell_ids = ?conductor.list_cell_ids(None));
    tracing::debug!(apps = ?conductor.list_running_apps().await.unwrap());

    display_agent_infos(&conductor).await;

    // Can't finish this test because there's no way to construct HolochainP2pEvents
    // and I can't directly call query on the conductor because it's private.

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn get_deleted() -> anyhow::Result<()> {
    observability::test_run().ok();
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;

    // Create a Conductor
    let mut conductor = SweetConductor::from_config(Default::default()).await;

    // Install DNA and install and enable apps in conductor
    let alice = conductor
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .into_cells()
        .into_iter()
        .next()
        .unwrap();
    // let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor
        .call(&alice.zome(SweetEasyInline::COORDINATOR), "create_unit", ())
        .await;
    let mut expected_count = WaitOps::start() + WaitOps::ENTRY;

    wait_for_integration_1m(alice.dht_db(), expected_count).await;

    let records: Vec<Option<Record>> = conductor
        .call(
            &alice.zome(SweetEasyInline::COORDINATOR),
            "read",
            hash.clone(),
        )
        .await;
    let record = records
        .into_iter()
        .next()
        .unwrap()
        .expect("Record was None: bobbo couldn't `get` it");

    assert_eq!(record.action().author(), alice.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );
    let entry_hash = record.action().entry_hash().unwrap();

    let _: ActionHash = conductor
        .call(
            &alice.zome(SweetEasyInline::COORDINATOR),
            "delete",
            hash.clone(),
        )
        .await;

    expected_count += WaitOps::DELETE;
    wait_for_integration_1m(alice.dht_db(), expected_count).await;

    let records: Vec<Option<Record>> = conductor
        .call(
            &alice.zome(SweetEasyInline::COORDINATOR),
            "read_entry",
            entry_hash,
        )
        .await;
    assert!(records.into_iter().next().unwrap().is_none());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn signal_subscription() {
    observability::test_run().ok();
    const N: usize = 10;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome())
        .await
        .unwrap();
    let mut conductor = SweetConductor::from_config(Default::default()).await;
    let app = conductor.setup_app("app", &[dna_file]).await.unwrap();
    let zome = &app.cells()[0].zome(SweetEasyInline::COORDINATOR);

    let signals = conductor.signals().take(N);

    // Emit N signals
    for _ in 0..N {
        let _: () = conductor.call(zome, "emit_signal", ()).await;
    }

    // Ensure that we can receive all signals
    let signals: Vec<Signal> = signals.collect().await;
    assert_eq!(signals.len(), N);
}

/// Simple zome which contains a validation rule which can fail
fn simple_validation_zome() -> InlineZomeSet {
    let entry_def = EntryDef::default_with_id("string");

    SweetEasyInline::new(vec![entry_def.clone()], 0)
        .callback("create", move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
        .integrity_callback("validate", |_api, data: Op| {
            let s = match data {
                Op::StoreEntry {
                    entry: Entry::App(bytes),
                    ..
                } => AppString::try_from(bytes.into_sb()).unwrap(),
                _ => return Ok(ValidateResult::Valid),
            };
            if &s.0 == "" {
                Ok(ValidateResult::Invalid("No empty strings allowed".into()))
            } else {
                Ok(ValidateResult::Valid)
            }
        })
        .0
}

#[tokio::test(flavor = "multi_thread")]
async fn simple_validation() -> anyhow::Result<()> {
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_validation_zome()).await?;
    let mut conductor = SweetConductor::from_standard_config().await;
    let (alice, bobbo) = SweetAgents::two(conductor.keystore()).await;
    let apps = conductor
        .setup_app_for_agents("app", &[alice.clone(), bobbo.clone()], &[dna_file])
        .await
        .unwrap();
    let ((alice,), (_bobbo,)) = apps.into_tuples();

    let alice = alice.zome(SweetEasyInline::COORDINATOR);

    // This call passes validation
    let h1: ActionHash = conductor.call(&alice, "create", AppString::new("A")).await;
    let e1s: Vec<Option<Record>> = conductor.call(&alice, "read", &h1).await;
    let e1 = e1s.into_iter().next().unwrap();
    let s1: AppString = e1.unwrap().entry().to_app_option().unwrap().unwrap();
    assert_eq!(s1, AppString::new("A"));

    // This call fails validation, and so results in an error
    let err: ConductorApiResult<ActionHash> = conductor
        .call_fallible(&alice, "create", AppString::new(""))
        .await;

    // This is kind of ridiculous, but we can't use assert_matches! because
    // there is a Box in the mix.
    let correct = match err {
        Err(ConductorApiError::CellError(e)) => match e {
            CellError::WorkflowError(e) => match *e {
                WorkflowError::SourceChainError(e) => match e {
                    SourceChainError::InvalidCommit(reason) => {
                        &reason == "No empty strings allowed"
                    }
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        },
        _ => false,
    };
    assert!(correct);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn can_call_real_zomes_too() {
    observability::test_run().ok();

    let mut conductor = SweetConductor::from_standard_config().await;
    let agent = SweetAgents::one(conductor.keystore()).await;
    let (mut integrity, mut coordinator) = simple_crud_zome().into_zomes();
    integrity.push(TestWasm::Create.into());
    coordinator.push(TestWasm::Create.into());

    let (dna, _, _) =
        SweetDnaFile::unique_from_zomes(integrity, coordinator, TestWasm::Create.into())
            .await
            .unwrap();

    let app = conductor
        .setup_app_for_agent("app1", agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let (cell,) = app.into_tuple();

    let hash: ActionHash = conductor
        .call(&cell.zome(SweetEasyInline::COORDINATOR), "create_unit", ())
        .await;

    let el: Option<Record> = conductor
        .call(&cell.zome("create_entry"), "get_post", hash.clone())
        .await;
    assert_eq!(el.unwrap().action_address(), &hash)
}

#[tokio::test(flavor = "multi_thread")]
/// Test that records can be manually inserted into a source chain.
async fn insert_source_chain() {
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome())
        .await
        .unwrap();
    let mut conductor = SweetConductor::from_standard_config().await;
    let apps = conductor
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap();
    let (alice,) = apps.into_tuple();

    let zome = alice.zome(SweetEasyInline::COORDINATOR);

    // Trigger init.
    let _: Vec<Option<Record>> = conductor
        .call(
            &zome,
            "read_entry",
            EntryHash::from(alice.cell_id().agent_pubkey().clone()),
        )
        .await;

    // Get the current chain source chain.
    let get_chain = |env| {
        fresh_reader_test(env, |txn| {
            let chain: Vec<(ActionHash, u32)> = txn
                .prepare("SELECT hash, seq FROM Action ORDER BY seq")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            chain
        })
    };

    // Get the source chain.
    let chain = get_chain(alice.authored_db().clone());
    let original_records: Vec<_> = fresh_reader_test(alice.authored_db().clone(), |txn| {
        let txn: Txn = (&txn).into();
        chain
            .iter()
            .map(|h| txn.get_record(&h.0.clone().into()).unwrap().unwrap())
            .collect()
    });
    // Chain should be 4 long.
    assert_eq!(chain.len(), 4);
    // Last seq should be 3.
    assert_eq!(chain.last().unwrap().1, 3);

    // Inject an action with the wrong author.
    let entry = Entry::app(().try_into().unwrap()).unwrap();
    let mut action = Create {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        action_seq: 4,
        prev_action: chain.last().unwrap().0.clone(),
        entry_type: EntryType::App(AppEntryType::new(
            1.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: EntryHash::with_data_sync(&entry),
        weight: Default::default(),
    };
    let shh = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action.clone().into()),
        fixt!(Signature),
    );
    let record = Record::new(shh, Some(entry.clone()));
    let result = conductor
        .clone()
        .insert_records_into_source_chain(alice.cell_id().clone(), false, false, vec![record])
        .await;
    // This gets rejected.
    assert!(matches!(
        result,
        Err(ConductorApiError::StateMutationError(
            StateMutationError::AuthorsMustMatch
        ))
    ));

    // Insert with correct author.
    action.author = alice.agent_pubkey().clone();

    let record = make_record(&conductor.keystore(), action.clone().into()).await;
    let hash = record.action_address().clone();
    conductor
        .clone()
        .insert_records_into_source_chain(alice.cell_id().clone(), false, false, vec![record])
        .await
        .expect("Should pass with valid agent");

    let chain = get_chain(alice.authored_db().clone());
    // Chain should be 5 long.
    assert_eq!(chain.len(), 5);
    // Last action should be the one we just inserted.
    assert_eq!(chain.last().unwrap().0, hash);

    // Make the action a fork
    action.action_seq = 3;
    action.prev_action = chain[2].0.clone();

    let record = make_record(&conductor.keystore(), action.clone().into()).await;
    let hash = record.action_address().clone();
    let result = conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            false,
            false,
            vec![record.clone()],
        )
        .await;

    // Validation is off so forking is possible.
    assert!(result.is_ok());

    let chain = get_chain(alice.authored_db().clone());
    // Chain should be 6 long.
    assert_eq!(chain.len(), 6);
    // The new action will be in the chain
    assert!(chain.iter().any(|i| i.0 == hash));

    // Insert with truncation on.
    let result = conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            true,
            false,
            vec![record.clone()],
        )
        .await;

    // An invalid chain is still possible because validation is off.
    // Note this cell is now in an invalid state.
    assert!(result.is_ok());

    let chain = get_chain(alice.authored_db().clone());
    // Chain should be 1 long.
    assert_eq!(chain.len(), 1);
    // The new action will be in the chain
    assert!(chain.iter().any(|i| i.0 == hash));

    // Restore the original records
    let result = conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            true,
            false,
            original_records.clone(),
        )
        .await;

    assert!(result.is_ok());
    let chain = get_chain(alice.authored_db().clone());
    // Chain should be 4 long.
    assert_eq!(chain.len(), 4);
    // Last seq should be 3.
    assert_eq!(chain.last().unwrap().1, 3);

    // Make the action a fork
    action.action_seq = 2;
    action.prev_action = chain[1].0.clone();
    let record = make_record(&conductor.keystore(), action.clone().into()).await;

    // Insert an invalid action with validation on.
    let result = conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            false,
            true,
            vec![record.clone()],
        )
        .await;

    // Fork is detected
    assert!(result.is_err());

    // Restore and validate the original records
    conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            true,
            true,
            original_records.clone(),
        )
        // Restoring the original records is ok because they
        // will pass validation.
        .await
        .expect("Should restore original chain");

    // Start a second conductor.
    let mut conductor = SweetConductor::from_standard_config().await;

    // The dna needs to be installed first.
    conductor.register_dna(dna_file.clone()).await.unwrap();

    // Insert the chain from the original conductor.
    conductor
        .clone()
        .insert_records_into_source_chain(
            alice.cell_id().clone(),
            true,
            true,
            original_records.clone(),
        )
        .await
        .expect("Can cold start");

    let apps = conductor
        .setup_app_for_agent("cold_start", alice.agent_pubkey().clone(), &[dna_file])
        .await
        .unwrap();
    let (alice_backup,) = apps.into_tuple();
    let chain = get_chain(alice_backup.authored_db().clone());
    // Chain should be 4 long.
    assert_eq!(chain.len(), 4);
    // Last seq should be 3.
    assert_eq!(chain.last().unwrap().1, 3);
}

async fn make_record(keystore: &MetaLairClient, action: Action) -> Record {
    let shh = SignedActionHashed::sign(
        keystore,
        ActionHashed::from_content_sync(action.clone().into()),
    )
    .await
    .unwrap();
    let entry = Entry::app(().try_into().unwrap()).unwrap();
    Record::new(shh, Some(entry.clone()))
}

/// Simple scenario involving two agents using the same DNA
#[tokio::test(flavor = "multi_thread")]
async fn call_non_existing_zome_fails_gracefully() -> anyhow::Result<()> {
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await?;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Get two agents
    let agent = SweetAgents::one(conductor.keystore()).await;

    // Install DNA and install and enable apps in conductor
    let app = conductor
        .setup_app_for_agent("app1", agent.clone(), &[dna_file.clone()])
        .await
        .unwrap();

    let (alice,) = app.into_tuple();

    // Call the a zome fn on a non existing zome on Alice's app
    let result: ConductorApiResult<ActionHash> = conductor
        .call_fallible(&alice.zome("non_existing_zome"), "create_unit", ())
        .await;

    assert_matches!(result, Err(_));

    Ok(())
}
