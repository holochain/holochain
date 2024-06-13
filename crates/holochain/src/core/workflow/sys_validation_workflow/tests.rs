use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::sweettest::SweetConductorBatch;
use crate::sweettest::SweetConductorConfig;
use crate::sweettest::SweetDnaFile;
use crate::sweettest::SweetInlineZomes;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use hdk::prelude::LinkTag;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn sys_validation_workflow_test() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let config = SweetConductorConfig::standard().no_dpki_mustfix();
    let mut conductors = SweetConductorBatch::from_config(2, config).await;
    let apps = conductors
        .setup_app(&"test_app", [&dna_file])
        .await
        .unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    conductors.exchange_peer_info().await;

    run_test(alice_cell_id, bob_cell_id, conductors, dna_file).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_warrants() {
    holochain_trace::test_run();
    let zome = SweetInlineZomes::new(vec![], 0);
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let ((alice,), (bob,)) = conductors
        .setup_app("app", [&dna])
        .await
        .unwrap()
        .into_tuples();
    let alice_pubkey = alice.agent_pubkey().clone();

    // - Create an invalid op
    let mut action = ::fixt::fixt!(CreateLink);
    action.author = alice_pubkey.clone();
    let action = Action::CreateLink(action);
    let signed_action =
        SignedActionHashed::sign(&conductors[0].keystore(), action.clone().into_hashed())
            .await
            .unwrap();
    let op = ChainOp::StoreRecord(
        signed_action.signature().clone(),
        action,
        RecordEntry::NotStored,
    )
    .into();
    let dna_def = dna.dna_def().clone().into_hashed();

    //- Check that the op is indeed invalid
    let outcome = crate::core::workflow::sys_validation_workflow::validate_op(
        &op,
        &dna_def,
        Default::default(),
        None,
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome, Outcome::Rejected(_));

    //- Inject the invalid op directly into bob's DHT db
    let op = DhtOpHashed::from_content_sync(op);
    let db = conductors[1].spaces.dht_db(dna.dna_hash()).unwrap();
    db.test_write(move |txn| {
        insert_op(txn, &op).unwrap();
    });

    //- Trigger sys validation
    conductors[1]
        .get_cell_triggers(bob.cell_id())
        .await
        .unwrap()
        .sys_validation
        .trigger(&"test");

    //- Check that bob authored a warrant
    crate::assert_eq_retry_1m!(
        {
            let basis: AnyLinkableHash = alice_pubkey.clone().into();
            conductors[1]
                .spaces
                .get_all_authored_dbs(dna.dna_hash())
                .unwrap()[0]
                .test_read(move |txn| {
                    let store = Txn::from(&txn);

                    let warrants = store.get_warrants_for_basis(&basis).unwrap();
                    warrants.len()
                })
        },
        1
    );
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    conductors: SweetConductorBatch,
    dna_file: DnaFile,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    bob_links_in_a_legit_way(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 9 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 9 + 14;

    let alice_dht_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_dht_db,
        expected_count,
        num_attempts,
        delay_per_attempt,
    )
    .await;

    let limbo_is_empty = |txn: &Transaction| {
        let not_empty: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        !not_empty
    };

    // holochain_state::prelude::dump_tmp(&alice_dht_db);
    // Validation should be empty
    alice_dht_db.read_async(move |txn| -> DatabaseResult<()> {
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        let num_valid_ops: usize = txn
                .query_row("SELECT COUNT(hash) FROM DhtOP WHERE when_integrated IS NOT NULL AND validation_status = :status",
                named_params!{
                    ":status": ValidationStatus::Valid,
                },
                |row| row.get(0))
                .unwrap();
        assert_eq!(num_valid_ops, expected_count);

        Ok(())
    }).await.unwrap();

    let (bad_update_action, bad_update_entry_hash, link_add_hash) =
        bob_makes_a_large_link(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 14 ops in it + the running tally
    let expected_count = 14 + expected_count;

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt).await;

    let bad_update_entry_hash: AnyDhtHash = bad_update_entry_hash.into();
    let num_valid_ops = move |txn: Transaction| -> DatabaseResult<usize> {
        let valid_ops: usize = txn
                .query_row(
                    "
                    SELECT COUNT(hash) FROM DhtOP
                    WHERE
                    when_integrated IS NOT NULL
                    AND
                    (validation_status = :valid
                        OR (validation_status = :rejected
                            AND (
                                (type = :store_entry AND basis_hash = :bad_update_entry_hash AND action_hash = :bad_update_action)
                                OR
                                (type = :store_record AND action_hash = :bad_update_action)
                                OR
                                (type = :add_link AND action_hash = :link_add_hash)
                                OR
                                (type = :update_content AND action_hash = :bad_update_action)
                                OR
                                (type = :update_record AND action_hash = :bad_update_action)
                            )
                        )
                    )
                    ",
                named_params!{
                    ":valid": ValidationStatus::Valid,
                    ":rejected": ValidationStatus::Rejected,
                    ":store_entry": ChainOpType::StoreEntry,
                    ":store_record": ChainOpType::StoreRecord,
                    ":add_link": ChainOpType::RegisterAddLink,
                    ":update_content": ChainOpType::RegisterUpdatedContent,
                    ":update_record": ChainOpType::RegisterUpdatedRecord,
                    ":bad_update_entry_hash": bad_update_entry_hash,
                    ":bad_update_action": bad_update_action,
                    ":link_add_hash": link_add_hash,
                },
                |row| row.get(0))
                .unwrap();

        Ok(valid_ops)
    };

    alice_db
        .read_async(move |txn| -> DatabaseResult<()> {
            // Validation should be empty
            let limbo = show_limbo(&txn);
            assert!(limbo_is_empty(&txn), "{:?}", limbo);

            Ok(())
        })
        .await
        .unwrap();

    let valid_ops = alice_db.read_async(num_valid_ops.clone()).await.unwrap();
    assert_eq!(valid_ops, expected_count);
}

async fn bob_links_in_a_legit_way(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> ActionHash {
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let link_tag = LinkTag::from(vec![0; 256]);
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;
    // 3
    call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 4
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 5
    // Link the entries
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob_cell_id).await.unwrap();
    triggers
        .publish_dht_ops
        .trigger(&"bob_links_in_a_legit_way");
    link_add_address
}

async fn bob_makes_a_large_link(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (ActionHash, EntryHash, ActionHash) {
    let base = Post("Small time base".into());
    let target = Post("Spam it big time".into());
    let bad_update = Msg("This is not the msg you were looking for".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let bad_update_entry_hash = Entry::try_from(bad_update.clone()).unwrap().to_hash();

    let bytes = (0..MAX_TAG_SIZE + 1).map(|_| 0u8).collect::<Vec<_>>();
    let link_tag = LinkTag(bytes);

    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;

    // 6
    let original_action_address = call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 7
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 8
    // Commit a large action
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // 9
    // Commit a bad update entry
    let bad_update_action = call_data
        .update_entry(
            bad_update.clone().try_into().unwrap(),
            original_action_address,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger(&"bob_makes_a_large_link");
    (bad_update_action, bad_update_entry_hash, link_add_address)
}

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLite> {
    txn.prepare(
        "
        SELECT DhtOp.type, Action.hash, Action.blob, Action.author
        FROM DhtOp
        JOIN Action ON DhtOp.action_hash = Action.hash
        WHERE
        when_integrated IS NULL
    ",
    )
    .unwrap()
    .query_and_then([], |row| {
        let op_type: DhtOpType = row.get("type")?;
        match op_type {
            DhtOpType::Chain(op_type) => {
                let hash: ActionHash = row.get("hash")?;
                let action: SignedAction = from_blob(row.get("blob")?)?;
                Ok(ChainOpLite::from_type(op_type, hash, &action)?.into())
            }
            DhtOpType::Warrant(_) => {
                let warrant: SignedWarrant = from_blob(row.get("blob")?)?;
                let author: AgentPubKey = row.get("author")?;
                let (TimedWarrant(warrant, timestamp), signature) = warrant.into();
                Ok(WarrantOp::new(warrant, author, signature, timestamp).into())
            }
        }
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLite>>>()
    .unwrap()
}
