#![cfg(feature = "test_utils")]
#![cfg(feature = "chc")]

use ::fixt::prelude::*;
use hdk::prelude::*;
use holochain::conductor::api::error::ConductorApiError;
use holochain::sweettest::{DynSweetRendezvous, SweetConductor, SweetDnaFile, SweetInlineZomes};
use holochain::test_utils::inline_zomes::simple_crud_zome;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_keystore::MetaLairClient;
use holochain_sqlite::db::{DbKindAuthored, DbWrite};
use holochain_sqlite::error::DatabaseResult;
use holochain_state::prelude::{StateMutationError, Store, Txn};
use holochain_types::record::SignedActionHashedExt;

/// Test that records can be manually grafted onto a source chain.
#[tokio::test(flavor = "multi_thread")]
async fn grafting() {
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;
    let mut config = ConductorConfig::default();
    config.chc_url = Some(url2::Url2::parse(
        holochain::conductor::chc::CHC_LOCAL_MAGIC_URL,
    ));
    let mut conductor = SweetConductor::from_config(config.clone()).await;
    let keystore = conductor.keystore();

    let apps = conductor.setup_app("app", [&dna_file]).await.unwrap();
    let (alice,) = apps.into_tuple();

    let zome = alice.zome(SweetInlineZomes::COORDINATOR);

    // Trigger init.
    let _: Vec<Option<Record>> = conductor
        .call(
            &zome,
            "read_entry",
            EntryHash::from(alice.cell_id().agent_pubkey().clone()),
        )
        .await;

    // Get the current chain source chain.
    let get_chain = |env: DbWrite<DbKindAuthored>| async move {
        env.read_async(move |txn| -> DatabaseResult<Vec<(ActionHash, u32)>> {
            let chain: Vec<(ActionHash, u32)> = txn
                .prepare("SELECT hash, seq FROM Action ORDER BY seq")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            Ok(chain)
        })
        .await
        .unwrap()
    };

    // Get the source chain.
    let chain = get_chain(alice.authored_db().clone()).await;
    let original_records: Vec<_> = alice
        .authored_db()
        .read_async({
            let query_chain = chain.clone();

            move |txn| -> DatabaseResult<Vec<_>> {
                let txn: Txn = (&txn).into();
                Ok(query_chain
                    .iter()
                    .map(|h| txn.get_record(&h.0.clone().into()).unwrap().unwrap())
                    .collect())
            }
        })
        .await
        .unwrap();
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
        entry_type: EntryType::App(AppEntryDef::new(
            1.into(),
            0.into(),
            EntryVisibility::Public,
        )),
        entry_hash: EntryHash::with_data_sync(&entry),
        weight: Default::default(),
    };
    let sah = SignedActionHashed::with_presigned(
        ActionHashed::from_content_sync(action.clone().into()),
        fixt!(Signature),
    );
    let record = Record::new(sah, Some(entry.clone()));
    let result = conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), false, vec![record])
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
        .graft_records_onto_source_chain(alice.cell_id().clone(), false, vec![record])
        .await
        .expect("Should pass with valid agent");

    let chain = get_chain(alice.authored_db().clone()).await;
    // Chain should be 5 long.
    assert_eq!(chain.len(), 5);
    // Last action should be the one we just grafted.
    assert_eq!(chain.last().unwrap().0, hash);

    // Make the action a fork
    action.action_seq = 3;
    action.prev_action = chain[2].0.clone();

    let record = make_record(&conductor.keystore(), action.clone().into()).await;
    let hash = record.action_address().clone();
    let result = conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), false, vec![record.clone()])
        .await;

    // Validation is off so forking is possible.
    assert!(result.is_ok());

    let chain = get_chain(alice.authored_db().clone()).await;
    // Chain should be 4 long, since the previous fork was cut off
    assert_eq!(chain.len(), 4);
    // The new action will be in the chain
    assert!(chain.iter().any(|i| i.0 == hash));

    // Graft records.
    let result = conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), false, vec![record.clone()])
        .await;

    // An invalid chain is still possible because validation is off.
    // Note this cell is now in an invalid state.
    assert!(result.is_ok());

    let chain2 = get_chain(alice.authored_db().clone()).await;
    // The chain is unchanged from adding the same action back in.
    assert_eq!(chain, chain2);

    // Restore the original records
    let result = conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), false, original_records.clone())
        .await;

    assert!(result.is_ok());
    let chain = get_chain(alice.authored_db().clone()).await;
    // Chain should be 4 long.
    assert_eq!(chain.len(), 4);
    // Last seq should be 3.
    assert_eq!(chain.last().unwrap().1, 3);

    // Make the action a fork
    action.action_seq = 2;
    action.prev_action = chain[1].0.clone();
    action.timestamp = Timestamp::from_micros(0);
    let record = make_record(&conductor.keystore(), action.clone().into()).await;

    // Insert an invalid action with validation on.
    let result = conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), true, vec![record.clone()])
        .await;

    // Fork is detected
    assert!(dbg!(result).is_err());

    // Restore and validate the original records
    // (there has been no change at this point, but it helps for clarity to reset the chain anyway)
    conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), true, original_records.clone())
        // Restoring the original records is ok because they
        // will pass validation.
        .await
        .expect("Should restore original chain");

    // Start a second conductor.
    let conductor =
        SweetConductor::create_with_defaults(config, Some(keystore), None::<DynSweetRendezvous>)
            .await;

    // The dna needs to be installed first.
    conductor.register_dna(dna_file.clone()).await.unwrap();

    let mut payload = holochain::sweettest::get_install_app_payload_from_dnas(
        "app",
        alice.agent_pubkey().clone(),
        &[(dna_file, None)],
    )
    .await;

    // This results in an error since the CHC already contains genesis, but this
    // is just to create the necessary cell for grafting onto.
    payload.ignore_genesis_failure = true;
    let install_result = conductor.raw_handle().install_app_bundle(payload).await;
    assert!(install_result.is_err());

    // Insert the chain from the original conductor.
    conductor
        .clone()
        .graft_records_onto_source_chain(alice.cell_id().clone(), true, original_records.clone())
        .await
        .expect("Can cold start");

    let chain = get_chain(alice.authored_db().clone()).await;
    // Chain should be 4 long.
    assert_eq!(chain.len(), 4);
    // Last seq should be 3.
    assert_eq!(chain.last().unwrap().1, 3);
}

async fn make_record(keystore: &MetaLairClient, action: Action) -> Record {
    let sah = SignedActionHashed::sign(keystore, ActionHashed::from_content_sync(action.clone()))
        .await
        .unwrap();
    let entry = Entry::app(().try_into().unwrap()).unwrap();
    Record::new(sah, Some(entry.clone()))
}
