#![cfg(feature = "test_utils")]

use hdk::prelude::*;
use holochain::core::ribosome::guest_callback::validate::ValidateResult;
use holochain::core::SourceChainError;
use holochain::test_utils::inline_zomes::{simple_crud_zome, AppString};
use holochain::{conductor::api::error::ConductorApiResult, sweettest::*};
use holochain::{
    conductor::{api::error::ConductorApiError, CellError},
    core::workflow::WorkflowError,
};
use holochain_types::{inline_zome::InlineZomeSet, prelude::*};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{op::Op, record::RecordEntry};
use matches::assert_matches;

/// Simple scenario involving two agents using the same DNA
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn inline_zome_2_agents_1_dna() -> anyhow::Result<()> {
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Install DNA and install and enable apps in conductor
    let apps = conductor.setup_apps("app", 2, &[dna_file]).await.unwrap();

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_unit",
            (),
        )
        .await;

    await_consistency(10, [&alice, &bobbo]).await.unwrap();

    // Verify that bobbo can run "read" on his cell and get alice's Action
    let records: Option<Record> = conductor
        .call(&bobbo.zome(SweetInlineZomes::COORDINATOR), "read", hash)
        .await;
    let record = records.expect("Record was None: bobbo couldn't `get` it");

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
    holochain_trace::test_run();
    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_foo, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let (dna_bar, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let apps = conductor
        .setup_apps("app", 3, &[dna_foo, dna_bar])
        .await
        .unwrap();

    let ((alice_foo, alice_bar), (bobbo_foo, bobbo_bar), (carol_foo, carol_bar)) =
        apps.into_tuples();

    assert_eq!(alice_foo.agent_pubkey(), alice_bar.agent_pubkey());
    assert_eq!(bobbo_foo.agent_pubkey(), bobbo_bar.agent_pubkey());
    assert_ne!(alice_foo.agent_pubkey(), bobbo_foo.agent_pubkey());

    //////////////////////
    // END SETUP

    let hash_foo: ActionHash = conductor
        .call(
            &alice_foo.zome(SweetInlineZomes::COORDINATOR),
            "create_unit",
            (),
        )
        .await;
    let hash_bar: ActionHash = conductor
        .call(
            &alice_bar.zome(SweetInlineZomes::COORDINATOR),
            "create_unit",
            (),
        )
        .await;

    // Two different DNAs, so ActionHashes should be different.
    assert_ne!(hash_foo, hash_bar);

    // Wait long enough for others to receive gossip
    await_consistency(10, [&alice_foo, &bobbo_foo, &carol_foo])
        .await
        .unwrap();
    await_consistency(10, [&alice_bar, &bobbo_bar, &carol_bar])
        .await
        .unwrap();

    // Verify that bobbo can run "read" on his cell and get alice's Action
    // on the "foo" DNA
    let record: Option<Record> = conductor
        .call(
            &bobbo_foo.zome(SweetInlineZomes::COORDINATOR),
            "read",
            hash_foo,
        )
        .await;
    let record = record.expect("Record was None: bobbo couldn't `get` it");
    assert_eq!(record.action().author(), alice_foo.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    // Verify that carol can run "read" on her cell and get alice's Action
    // on the "bar" DNA
    // Let's do it with the SweetZome instead of the SweetCell too, for fun
    let record: Option<Record> = conductor
        .call(
            &carol_bar.zome(SweetInlineZomes::COORDINATOR),
            "read",
            hash_bar,
        )
        .await;
    let record = record.expect("Record was None: carol couldn't `get` it");
    assert_eq!(record.action().author(), alice_bar.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn get_deleted() -> anyhow::Result<()> {
    holochain_trace::test_run();
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Install DNA and install and enable apps in conductor
    let alice = conductor
        .setup_app("app", &[dna_file])
        .await
        .unwrap()
        .into_cells()
        .into_iter()
        .next()
        .unwrap();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductor
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_unit",
            (),
        )
        .await;

    await_consistency(10, [&alice]).await.unwrap();

    let records: Option<Record> = conductor
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "read",
            hash.clone(),
        )
        .await;
    let record = records.expect("Record was None: bobbo couldn't `get` it");

    assert_eq!(record.action().author(), alice.agent_pubkey());
    assert_eq!(
        *record.entry(),
        RecordEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );
    let entry_hash = record.action().entry_hash().unwrap();

    let _: ActionHash = conductor
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "delete",
            hash.clone(),
        )
        .await;

    await_consistency(10, [&alice]).await.unwrap();

    let records: Vec<Option<Record>> = conductor
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
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
    holochain_trace::test_run();
    const N: usize = 10;

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app = conductor.setup_app("app", &[dna_file]).await.unwrap();
    let zome = &app.cells()[0].zome(SweetInlineZomes::COORDINATOR);

    let mut signal_rx = conductor.subscribe_to_app_signals("app".to_string());

    // Emit N signals
    for _ in 0..N {
        let _: () = conductor.call(zome, "emit_signal", ()).await;
    }

    // Ensure that we can receive all signals
    let mut signals: Vec<Signal> = vec![];
    for _ in 0..N {
        let signal = signal_rx.recv().await.unwrap();
        signals.push(signal);
    }

    assert_eq!(signals.len(), N);
}

/// Simple zome which contains a validation rule which can fail
fn simple_validation_zome() -> InlineZomeSet {
    let entry_def = EntryDef::default_from_id("string");

    SweetInlineZomes::new(vec![entry_def.clone()], 0)
        .function("create", move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
        .integrity_function("validate", |_api, data: Op| {
            let s = match data {
                Op::StoreEntry(StoreEntry {
                    entry: Entry::App(bytes),
                    ..
                }) => AppString::try_from(bytes.into_sb()).unwrap(),
                _ => return Ok(ValidateResult::Valid),
            };
            if s.0.is_empty() {
                Ok(ValidateResult::Invalid("No empty strings allowed".into()))
            } else {
                Ok(ValidateResult::Valid)
            }
        })
        .0
}

#[tokio::test(flavor = "multi_thread")]
async fn simple_validation() -> anyhow::Result<()> {
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_validation_zome()).await;
    let mut conductor = SweetConductor::from_standard_config().await;

    let apps = conductor.setup_apps("app", 2, &[dna_file]).await.unwrap();
    let ((alice,), (_bobbo,)) = apps.into_tuples();

    let alice = alice.zome(SweetInlineZomes::COORDINATOR);

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
        Err(ConductorApiError::CellError(CellError::WorkflowError(ref e))) => match &**e {
            WorkflowError::SourceChainError(SourceChainError::InvalidCommit(reason)) => {
                reason.contains("No empty strings allowed")
            }
            _ => false,
        },
        _ => false,
    };
    assert!(correct, "Error was: {:?}", err);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn can_call_real_zomes_too() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;
    let (mut integrity, mut coordinator) = simple_crud_zome().into_zomes();
    integrity.push(TestWasm::Create.into());
    coordinator.push(TestWasm::Create.into());

    let (dna, _, _) =
        SweetDnaFile::unique_from_zomes(integrity, coordinator, TestWasm::Create.into()).await;

    let app = conductor.setup_app("app1", &[dna.clone()]).await.unwrap();

    let (cell,) = app.into_tuple();

    let hash: ActionHash = conductor
        .call(&cell.zome(SweetInlineZomes::COORDINATOR), "create_unit", ())
        .await;

    let el: Option<Record> = conductor
        .call(&cell.zome("create_entry"), "get_post", hash.clone())
        .await;
    assert_eq!(el.unwrap().action_address(), &hash)
}

/// Simple scenario involving two agents using the same DNA
#[tokio::test(flavor = "multi_thread")]
async fn call_non_existing_zome_fails_gracefully() -> anyhow::Result<()> {
    // Bundle the single zome into a DnaFile
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    // Install DNA and install and enable apps in conductor
    let app = conductor.setup_app("app1", [&dna_file]).await.unwrap();

    let (alice,) = app.into_tuple();

    // Call the a zome fn on a non existing zome on Alice's app
    let result: ConductorApiResult<ActionHash> = conductor
        .call_fallible(&alice.zome("non_existing_zome"), "create_unit", ())
        .await;

    assert_matches!(result, Err(_));

    Ok(())
}
