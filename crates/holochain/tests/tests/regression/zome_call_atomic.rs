use hdk::prelude::{Entry, EntryDefIndex, EntryVisibility, ExternIO};
use holo_hash::ActionHash;
use holochain::sweettest::{SweetConductor, SweetDnaFile};
use holochain_types::inline_zome::InlineZomeSet;
use holochain_types::prelude::Record;
use holochain_types::signal::Signal;
use holochain_zome_types::action::ChainTopOrdering;
use holochain_zome_types::entry::{CreateInput, GetInput, GetOptions};
use holochain_zome_types::prelude::{EntryDef, InlineZomeError};
use holochain_zome_types::signal::AppSignal;

/// When there is any error in a zome call, any data in the scratch space should not be written to
/// the agent's source chain.
#[tokio::test(flavor = "multi_thread")]
async fn zome_call_error_drop_uncommited() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let zome = InlineZomeSet::new_unique_single(
        "integrity",
        "coordinator",
        vec![EntryDef::default_from_id("1")],
        0,
    )
    .function::<_, _, ()>("coordinator", "create_with_error", move |api, ()| {
        let entry = Entry::app(().try_into().unwrap()).unwrap();
        let hash = api.create(CreateInput::new(
            InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
            EntryVisibility::Public,
            entry,
            ChainTopOrdering::default(),
        ))?;
        api.emit_signal(AppSignal::new(ExternIO::encode(hash)?))?;

        Err(InlineZomeError::TestError(
            "Something went wrong".to_string(),
        ))
    })
    .function(
        "coordinator",
        "get_by_hash",
        move |api, hash: ActionHash| {
            let entry = api.get(vec![GetInput::new(hash.into(), GetOptions::local())])?;
            Ok(entry[0].clone())
        },
    );

    let dna = SweetDnaFile::unique_from_inline_zomes(zome).await;

    let app = conductor.setup_app("app", &[dna.0]).await.unwrap();
    let cell = app.into_cells()[0].clone();

    let mut app_signal = conductor.subscribe_to_app_signals("app".into());

    let err = conductor
        .call_fallible::<_, ()>(&cell.zome("coordinator"), "create_with_error", ())
        .await
        .unwrap_err();

    let msg = app_signal.recv().await.unwrap();
    let action_hash = match msg {
        Signal::App { signal, .. } => {
            let action_hash: ActionHash = signal.into_inner().decode().unwrap();
            action_hash
        }
        _ => panic!("Expected AppSignal, got {msg:?}"),
    };

    let entry = conductor
        .call::<_, Option<Record>>(&cell.zome("coordinator"), "get_by_hash", action_hash)
        .await;

    assert!(
        entry.is_none(),
        "Entry should not have been created due to error: {err:?}"
    );
}
