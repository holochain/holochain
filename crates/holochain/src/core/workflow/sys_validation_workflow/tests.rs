use super::*;
use crate::core::workflow::sys_validation_workflow::types::Outcome;
use crate::sweettest::*;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::inline_zomes::simple_crud_zome;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
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

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    conductors.exchange_peer_info().await;

    run_test(alice_cell_id, bob_cell_id, conductors, dna_file).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_invalid_chain_warrant() {
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

                    let warrants = store.get_warrants_for_basis(&basis, false).unwrap();
                    warrants.len()
                })
        },
        1
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_forked_chain_warrant() {
    holochain_trace::test_run();
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(simple_crud_zome()).await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let ((alice,), (bob,)) = conductors
        .setup_app("app", [&dna])
        .await
        .unwrap()
        .into_tuples();
    let alice_pubkey = alice.agent_pubkey().clone();
    let bob_pubkey = bob.agent_pubkey().clone();

    // For this test we want bob to get alice's chain so he can detect the fork
    conductors.exchange_peer_info().await;

    let action_hash: ActionHash = conductors[0]
        .call(&alice.zome("coordinator"), "create_unit", ())
        .await;
    let records: Option<Record> = conductors[0]
        .call(&alice.zome("coordinator"), "read", action_hash)
        .await;

    //- Modify the just-created record to produce a chain fork
    let record = records.unwrap();
    let (action, _) = record.into_inner();
    let mut action = action.into_inner().0.into_content();
    let entry = Entry::App(
        AppEntryBytes::try_from(SerializedBytes::from(UnsafeBytes::from(vec![1, 2, 3]))).unwrap(),
    );
    *action.entry_data_mut().unwrap().0 = entry.to_hash();
    let action = SignedActionHashed::sign(&conductors[0].keystore(), action.into_hashed())
        .await
        .unwrap();
    let (action, signature) = action.into_inner();
    let action = SignedAction::new(action.into_content(), signature);
    let forked_op = ChainOp::from_type(ChainOpType::StoreRecord, action, Some(entry)).unwrap();

    //- Check that the op is valid
    let dna_def = dna.dna_def().clone().into_hashed();
    let outcome = crate::core::workflow::sys_validation_workflow::validate_op(
        &forked_op.clone().into(),
        &dna_def,
        Default::default(),
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome, Outcome::Accepted);

    await_consistency(10, [&alice, &bob]).await.unwrap();

    //- Inject the forked op directly into bob's DHT db
    let forked_op = DhtOpHashed::from_content_sync(forked_op);
    let db = conductors[1].spaces.dht_db(dna.dna_hash()).unwrap();
    db.test_write(move |txn| {
        insert_op(txn, &forked_op).unwrap();
    });

    //- Trigger sys validation
    conductors[1]
        .get_cell_triggers(bob.cell_id())
        .await
        .unwrap()
        .sys_validation
        .trigger(&"test");

    //- Check that bob authored a chain fork warrant
    crate::wait_for_1m!(
        {
            let basis: AnyLinkableHash = alice_pubkey.clone().into();
            conductors[1]
                .spaces
                .get_or_create_authored_db(dna.dna_hash(), bob_pubkey.clone())
                .unwrap()
                .test_read(move |txn| {
                    let store = Txn::from(&txn);
                    store.get_warrants_for_basis(&basis, false).unwrap()
                })
        },
        |warrants: &Vec<Warrant>| { !warrants.is_empty() },
        |mut warrants: Vec<Warrant>| {
            matches::assert_matches!(
                warrants.pop().unwrap().proof,
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork { .. })
            )
        }
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
                Ok(warrant.into())
            }
        }
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLite>>>()
    .unwrap()
}

/// Test the detect_fork function against different situations,
/// especially the case where a fork happens after an Update Agent action,
/// where the authorship changes
#[tokio::test(flavor = "multi_thread")]
async fn test_detect_fork() {
    use ::fixt::fixt;
    let keystore = holochain_keystore::test_keystore();
    let author1 = keystore.new_sign_keypair_random().await.unwrap();
    let author2 = keystore.new_sign_keypair_random().await.unwrap();

    let sign_action = |a: Action| async {
        SignedActionHashed::sign(&keystore, a.into_hashed())
            .await
            .unwrap()
    };
    let basic_action = |author: AgentPubKey, prev: Option<ActionHash>| {
        if let Some(prev) = prev {
            let mut a = fixt!(Create);
            a.entry_type = EntryType::App(fixt!(AppEntryDef));
            a.author = author;
            a.prev_action = prev;
            Action::Create(a)
        } else {
            let mut a = fixt!(Dna);
            a.author = author;
            Action::Dna(a)
        }
    };

    // - Two actions, one following the other
    let a0 = basic_action(author1.clone(), None);
    let a1 = basic_action(author1.clone(), Some(a0.to_hash()));

    // - Create an agent key update following a1
    let mut update = fixt!(Update);
    update.author = author1.clone();
    update.entry_type = EntryType::AgentPubKey;
    update.entry_hash = author2.clone().into();
    update.prev_action = a1.to_hash();
    let a2 = Action::Update(update);

    // - Two more actions following a2
    let a3 = basic_action(author2.clone(), Some(a2.to_hash()));
    let a4 = basic_action(author2.clone(), Some(a3.to_hash()));

    // - Create a forked version of a1 (still pointing to a0)
    let mut a1_fork = a1.clone();
    *a1_fork.entry_data_mut().unwrap().0 = fixt!(EntryHash);

    // - Create a forked version of a3 (still pointing to a2)
    let mut a3_fork = a3.clone();
    *a3_fork.entry_data_mut().unwrap().0 = fixt!(EntryHash);

    // - Create another forked version of a3, with the pre-update author
    let mut a3_fork_author1 = a3.clone();
    *a3_fork_author1.author_mut() = author1.clone();
    *a3_fork_author1.entry_data_mut().unwrap().0 = fixt!(EntryHash);

    // - Create another forked version of a3, with a random author
    let mut a3_fork_other_author = a3.clone();
    *a3_fork_other_author.author_mut() = fixt!(AgentPubKey);
    *a3_fork_other_author.entry_data_mut().unwrap().0 = fixt!(EntryHash);

    let a1_hash = a1.to_hash();
    let a3_hash = a3.to_hash();

    // - Form a chain of the "valid, unforked" actions
    let chain = [
        sign_action(a0).await,
        sign_action(a1).await,
        sign_action(a2).await,
        sign_action(a3.clone()).await,
    ];

    let db = test_authored_db();
    db.test_write(move |txn| {
        // - Commit the valid chain
        for a in chain {
            insert_action(txn, &a).unwrap();
        }

        // Not a fork, because a4 is a perfectly valid continuation of a3
        assert!(detect_fork(txn, &a4).unwrap().is_none());

        // Not a fork, because a3 is already in the chain
        assert!(detect_fork(txn, &a3).unwrap().is_none());

        // Is a fork, because:
        // - a1 already exists
        // - both actions point to the same previous action a0
        // - both are under the same authorship as a0
        assert_eq!(detect_fork(txn, &a1_fork).unwrap().unwrap().0, a1_hash);

        // Is a fork, because:
        // - a3 already exists
        // - both actions point to the same previous action a2
        // - both are under the authorship of the key which a2 updates to
        assert_eq!(detect_fork(txn, &a3_fork).unwrap().unwrap().0, a3_hash);

        // This is not valid in sys validation because the author is not valid,
        // but it still does technically constitute a fork (it's just an invalid action)
        assert_eq!(
            detect_fork(txn, &a3_fork_author1).unwrap().unwrap().0,
            a3_hash
        );

        // This is not valid in sys validation because the author is not valid,
        // but it does still constitute a fork (it's just an invalid action)
        assert_eq!(
            detect_fork(txn, &a3_fork_other_author).unwrap().unwrap().0,
            a3_hash
        );
    });
}
