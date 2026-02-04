use super::*;
use crate::retry_until_timeout;
use crate::sweettest::*;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use holo_hash::fixt::{AgentPubKeyFixturator, EntryHashFixturator};
use holochain_types::test_utils::ActionRefMut;
use holochain_wasm_test_utils::TestWasm;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::convert::TryFrom;
use std::time::Duration;
use {
    crate::core::workflow::sys_validation_workflow::types::Outcome, ::fixt::fixt,
    holochain_zome_types::fixt::EntryFixturator, std::convert::TryInto,
};

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_workflow_test() {
    holochain_trace::test_run();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductors = SweetConductorBatch::standard(2).await;
    let apps = conductors.setup_app("test_app", [&dna_file]).await.unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    run_test(alice_cell_id, bob_cell_id, conductors, dna_file).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_invalid_chain_op_warrant() {
    use crate::test_utils::retry_fn_until_timeout;

    holochain_trace::test_run();
    let zome = SweetInlineZomes::new(vec![], 0);
    let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zome).await;

    let mut conductor = SweetConductor::standard().await;
    let alice = conductor.setup_app("app", [&dna]).await.unwrap();

    // - Create an invalid op
    let bob_pubkey = fixt!(AgentPubKey);
    let mut mismatched_action = fixt!(Create);
    mismatched_action.author = bob_pubkey.clone();
    let op = ChainOp::StoreEntry(
        fixt!(Signature),
        NewEntryAction::Create(mismatched_action),
        fixt!(Entry),
    )
    .into();
    let dna_hash = dna.dna_hash().clone();

    //- Check that the op is indeed invalid
    let outcome = crate::core::workflow::sys_validation_workflow::validate_op(
        &op,
        &dna_hash,
        Default::default(),
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome, Outcome::Rejected(_));

    //- Inject the invalid op directly into bob's DHT db
    let op = DhtOpHashed::from_content_sync(op);
    let db = conductor.spaces.dht_db(dna.dna_hash()).unwrap();
    db.test_write(move |txn| {
        insert_op_dht(txn, &op, 0, None).unwrap();
    });

    //- Trigger sys validation
    conductor
        .get_cell_triggers(alice.cells()[0].cell_id())
        .await
        .unwrap()
        .sys_validation
        .trigger(&"test");

    retry_fn_until_timeout(
        || async {
            let key = bob_pubkey.clone();
            let num_of_warrants = conductor
                .spaces
                .get_all_authored_dbs(dna.dna_hash())
                .unwrap()[0]
                .test_read(move |txn| {
                    let store = CascadeTxnWrapper::from(txn);

                    let warrants = store.get_warrants_for_agent(&key, false).unwrap();

                    warrants.len()
                });
            num_of_warrants == 1
        },
        Some(10000),
        None,
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_forked_chain_warrant() {
    holochain_trace::test_run();
    let keystore = holochain_keystore::test_keystore();

    // Create Alice's key (the forking agent)
    let alice_pubkey = keystore.new_sign_keypair_random().await.unwrap();

    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(crate::test_utils::inline_zomes::simple_crud_zome())
            .await;

    // Set up only Bob (we manually create Alice's ops to avoid gossip flakiness)
    let mut conductor = SweetConductor::standard().await;
    let bob = conductor.setup_app("app", [&dna]).await.unwrap();
    let bob_pubkey = bob.agent().clone();
    let bob_cell_id = bob.cells()[0].cell_id().clone();

    // Create Alice's genesis action (Dna action at seq 0)
    let mut dna_action = fixt!(Dna);
    dna_action.author = alice_pubkey.clone();
    let dna_action = Action::Dna(dna_action);
    let signed_dna_action = SignedActionHashed::sign(&keystore, dna_action.into_hashed())
        .await
        .unwrap();
    let prev_action_hash = signed_dna_action.as_hash().clone();

    // Create the original action at seq 1
    let original_entry = Entry::App(AppEntryBytes(UnsafeBytes::from(vec![1; 10]).into()));
    let mut original_create = fixt!(Create);
    original_create.author = alice_pubkey.clone();
    original_create.prev_action = prev_action_hash.clone();
    original_create.action_seq = 1;
    original_create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    original_create.entry_hash = original_entry.to_hash();

    // Create a forked action at seq 1 with a different entry
    let forked_entry = Entry::App(AppEntryBytes(UnsafeBytes::from(vec![2; 10]).into()));
    let mut forked_create = fixt!(Create);
    forked_create.author = alice_pubkey.clone();
    forked_create.prev_action = prev_action_hash.clone();
    forked_create.action_seq = 1;
    forked_create.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    forked_create.entry_hash = forked_entry.to_hash();

    let original_action = Action::Create(original_create);
    let forked_action = Action::Create(forked_create);

    let signed_original = SignedActionHashed::sign(&keystore, original_action.into_hashed())
        .await
        .unwrap();
    let signed_forked = SignedActionHashed::sign(&keystore, forked_action.into_hashed())
        .await
        .unwrap();

    let original_action_hash = signed_original.as_hash().clone();
    let forked_action_hash = signed_forked.as_hash().clone();
    let expected_seq = 1u32;

    // Build ChainOps for genesis, original, and forked actions
    let (dna_content, dna_sig) = signed_dna_action.into_inner();
    let dna_signed = SignedAction::new(dna_content.into_content(), dna_sig);
    let prev_op = ChainOp::from_type(ChainOpType::StoreRecord, dna_signed, None).unwrap();

    let (orig_content, orig_sig) = signed_original.into_inner();
    let orig_signed = SignedAction::new(orig_content.into_content(), orig_sig);
    let original_op =
        ChainOp::from_type(ChainOpType::StoreRecord, orig_signed, Some(original_entry)).unwrap();

    let (fork_content, fork_sig) = signed_forked.into_inner();
    let fork_signed = SignedAction::new(fork_content.into_content(), fork_sig);
    let forked_op =
        ChainOp::from_type(ChainOpType::StoreRecord, fork_signed, Some(forked_entry)).unwrap();

    // Verify the forked op is valid on its own
    let dna_hash = dna.dna_hash().clone();
    let outcome = crate::core::workflow::sys_validation_workflow::validate_op(
        &forked_op.clone().into(),
        &dna_hash,
        Default::default(),
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome, Outcome::Accepted);

    // Inject genesis + original action (as already-integrated data) and the
    // forked op (as pending validation) into Bob's DHT db
    let prev_op_hashed = DhtOpHashed::from_content_sync(prev_op);
    let original_op_hashed = DhtOpHashed::from_content_sync(original_op);
    let forked_op_hashed = DhtOpHashed::from_content_sync(forked_op);
    let db = conductor.spaces.dht_db(dna.dna_hash()).unwrap();
    db.test_write(move |txn| {
        insert_op_dht(txn, &prev_op_hashed, 0, None).unwrap();
        insert_op_dht(txn, &original_op_hashed, 0, None).unwrap();
        insert_op_dht(txn, &forked_op_hashed, 0, None).unwrap();
    });

    // Check that Bob authored a chain fork warrant with the correct action hashes
    retry_until_timeout!(60_000, 500, {
        // Trigger sys validation
        conductor
            .get_cell_triggers(&bob_cell_id)
            .await
            .unwrap()
            .sys_validation
            .trigger(&"test");

        let query_author = alice_pubkey.clone();
        let warrants: Vec<WarrantOp> = conductor
            .spaces
            .get_or_create_authored_db(dna.dna_hash(), bob_pubkey.clone())
            .unwrap()
            .test_read(move |txn| {
                let store = CascadeTxnWrapper::from(txn);
                store.get_warrants_for_agent(&query_author, false).unwrap()
            });

        if !warrants.is_empty() {
            let warrant = &warrants[0];
            match &warrant.proof {
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                    chain_author,
                    action_pair: ((hash1, _sig1), (hash2, _sig2)),
                    seq,
                }) => {
                    assert_eq!(chain_author, &alice_pubkey);
                    assert_eq!(*seq, expected_seq, "Warrant seq should match action seq");
                    let hashes = [hash1.clone(), hash2.clone()];
                    assert!(
                        hashes.contains(&original_action_hash),
                        "Warrant should contain original action hash"
                    );
                    assert!(
                        hashes.contains(&forked_action_hash),
                        "Warrant should contain forked action hash"
                    );
                }
                _ => panic!("Expected ChainFork warrant"),
            }
            break;
        }
    });
}

/// Test that when a peer receives both forked ops and runs sys validation,
/// they create 2 chain fork warrants (one when validating each op).
#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_produces_two_warrants_when_receiving_both_forked_ops() {
    holochain_trace::test_run();
    let keystore = holochain_keystore::test_keystore();

    // Create Alice's key (the forking agent)
    let alice_pubkey = keystore.new_sign_keypair_random().await.unwrap();

    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(crate::test_utils::inline_zomes::simple_crud_zome())
            .await;

    // Set up only Bob (we don't need Alice as a conductor since we're manually creating her ops)
    let mut conductor = SweetConductor::standard().await;
    let bob = conductor.setup_app("app", [&dna]).await.unwrap();
    let bob_pubkey = bob.agent().clone();
    let bob_cell_id = bob.cells()[0].cell_id().clone();

    // Create Alice's genesis action (Dna action at seq 0)
    let mut dna_action = fixt!(Dna);
    dna_action.author = alice_pubkey.clone();
    let dna_action = Action::Dna(dna_action);
    let signed_dna_action = SignedActionHashed::sign(&keystore, dna_action.into_hashed())
        .await
        .unwrap();
    let prev_action_hash = signed_dna_action.as_hash().clone();

    // Create two forked actions that both point to the same prev_action
    let entry1 = Entry::App(AppEntryBytes(UnsafeBytes::from(vec![1; 10]).into()));
    let entry2 = Entry::App(AppEntryBytes(UnsafeBytes::from(vec![2; 10]).into()));

    let mut create1 = fixt!(Create);
    create1.author = alice_pubkey.clone();
    create1.prev_action = prev_action_hash.clone();
    create1.action_seq = 1;
    create1.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create1.entry_hash = entry1.to_hash();

    let mut create2 = fixt!(Create);
    create2.author = alice_pubkey.clone();
    create2.prev_action = prev_action_hash.clone();
    create2.action_seq = 1;
    create2.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    create2.entry_hash = entry2.to_hash();

    let action1 = Action::Create(create1);
    let action2 = Action::Create(create2);

    let signed_action1 = SignedActionHashed::sign(&keystore, action1.into_hashed())
        .await
        .unwrap();
    let signed_action2 = SignedActionHashed::sign(&keystore, action2.into_hashed())
        .await
        .unwrap();

    let action1_hash = signed_action1.as_hash().clone();
    let action2_hash = signed_action2.as_hash().clone();

    // Create ChainOps for the previous action and both forked actions
    let (dna_action_content, dna_sig) = signed_dna_action.into_inner();
    let dna_signed_action = SignedAction::new(dna_action_content.into_content(), dna_sig);
    let prev_op = ChainOp::from_type(ChainOpType::StoreRecord, dna_signed_action, None).unwrap();

    let (action1_content, sig1) = signed_action1.into_inner();
    let signed_action1 = SignedAction::new(action1_content.into_content(), sig1);
    let op1 = ChainOp::from_type(ChainOpType::StoreRecord, signed_action1, Some(entry1)).unwrap();

    let (action2_content, sig2) = signed_action2.into_inner();
    let signed_action2 = SignedAction::new(action2_content.into_content(), sig2);
    let op2 = ChainOp::from_type(ChainOpType::StoreRecord, signed_action2, Some(entry2)).unwrap();

    // Verify both ops are valid on their own
    let dna_hash = dna.dna_hash().clone();
    let outcome1 = crate::core::workflow::sys_validation_workflow::validate_op(
        &op1.clone().into(),
        &dna_hash,
        Default::default(),
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome1, Outcome::Accepted);

    let outcome2 = crate::core::workflow::sys_validation_workflow::validate_op(
        &op2.clone().into(),
        &dna_hash,
        Default::default(),
    )
    .await
    .unwrap();
    matches::assert_matches!(outcome2, Outcome::Accepted);

    // Inject the previous action and both forked ops into Bob's DHT db
    let prev_op_hashed = DhtOpHashed::from_content_sync(prev_op);
    let op1_hashed = DhtOpHashed::from_content_sync(op1);
    let op2_hashed = DhtOpHashed::from_content_sync(op2);
    let db = conductor.spaces.dht_db(dna.dna_hash()).unwrap();
    db.test_write(move |txn| {
        insert_op_dht(txn, &prev_op_hashed, 0, None).unwrap();
        insert_op_dht(txn, &op1_hashed, 0, None).unwrap();
        insert_op_dht(txn, &op2_hashed, 0, None).unwrap();
    });

    // Check that Bob authored 2 chain fork warrants
    retry_until_timeout!(60_000, 500, {
        // Trigger sys validation
        conductor
            .get_cell_triggers(&bob_cell_id)
            .await
            .unwrap()
            .sys_validation
            .trigger(&"test");

        let query_author = alice_pubkey.clone();
        let warrants: Vec<WarrantOp> = conductor
            .spaces
            .get_or_create_authored_db(dna.dna_hash(), bob_pubkey.clone())
            .unwrap()
            .test_read(move |txn| {
                let store = CascadeTxnWrapper::from(txn);
                store.get_warrants_for_agent(&query_author, false).unwrap()
            });

        if warrants.len() == 2 {
            // Verify we have exactly 2 warrants
            assert_eq!(warrants.len(), 2, "Expected 2 chain fork warrants");

            // Collect all action hashes from both warrants
            let mut all_hashes = Vec::new();
            for warrant in &warrants {
                match &warrant.proof {
                    WarrantProof::ChainIntegrity(ChainIntegrityWarrant::ChainFork {
                        chain_author,
                        action_pair: ((hash1, _sig1), (hash2, _sig2)),
                        seq,
                    }) => {
                        // Verify chain_author matches alice
                        assert_eq!(chain_author, &alice_pubkey);
                        // Verify seq matches the forked actions' sequence number
                        assert_eq!(*seq, 1, "Warrant seq should match forked action seq");
                        all_hashes.push(hash1.clone());
                        all_hashes.push(hash2.clone());
                    }
                    _ => panic!("Expected ChainFork warrant"),
                }
            }

            // Both warrants should reference the same pair of forked actions
            assert!(
                all_hashes.contains(&action1_hash),
                "Warrants should contain action1 hash"
            );
            assert!(
                all_hashes.contains(&action2_hash),
                "Warrants should contain action2 hash"
            );
            break;
        }
    });
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
    .await
    .unwrap();

    let limbo_is_empty = |txn: &Transaction| {
        let not_empty: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOp WHERE when_integrated IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        !not_empty
    };

    // holochain_state::prelude::dump_tmp(&alice_dht_db);
    // Validation should be empty
    alice_dht_db.read_async(move |txn| -> DatabaseResult<()> {
        let limbo = show_limbo(txn);
        assert!(limbo_is_empty(txn), "{limbo:?}");

        let num_valid_ops: usize = txn
            .query_row("SELECT COUNT(hash) FROM DhtOp WHERE when_integrated IS NOT NULL AND validation_status = :status",
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

    // Integration should have 14 chain ops in it + 1 warrant op + the running tally
    let expected_count = 14 + 1 + expected_count;

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(&alice_db, expected_count, num_attempts, delay_per_attempt)
        .await
        .unwrap();

    let bad_update_entry_hash: AnyDhtHash = bad_update_entry_hash.into();
    let num_valid_ops = move |txn: &Transaction| -> DatabaseResult<usize> {
        let valid_ops: usize = txn
                .query_row(
                    "
                    SELECT COUNT(hash) FROM DhtOp
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

    let (limbo, empty) = alice_db
        .read_async(move |txn| {
            // Validation should be empty
            let limbo = show_limbo(txn);
            let empty = limbo_is_empty(txn);
            DatabaseResult::Ok((limbo, empty))
        })
        .await
        .unwrap();

    assert!(empty, "{limbo:?}");

    let valid_ops = alice_db
        .read_async(move |txn| num_valid_ops(txn))
        .await
        .unwrap();
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
    triggers
        .integrate_dht_ops
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
    triggers
        .integrate_dht_ops
        .trigger(&"bob_makes_a_large_link");
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

        // Not a fork: DNA actions have no prev_action, so the SQL query `prev_hash = :prev_hash`
        // with NULL returns no rows (since NULL = NULL is NULL in SQL, not true).
        // Create a different DNA action to ensure it doesn't match any existing action.
        let mut another_dna = fixt!(Dna);
        another_dna.author = author1.clone();
        let another_dna_action = Action::Dna(another_dna);
        assert!(
            detect_fork(txn, &another_dna_action).unwrap().is_none(),
            "DNA actions cannot fork - they have no prev_action"
        );

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

        // Error: a3_fork_author1 has author1 but the existing a3 in the DB
        // has author2. The in-memory author check detects a cross-author
        // prev_action collision and returns an error.
        assert!(
            detect_fork(txn, &a3_fork_author1).is_err(),
            "Cross-author prev_action collision should return an error"
        );

        // Error: a3_fork_other_author has a random author that doesn't match
        // any existing action with the same prev_hash.
        assert!(
            detect_fork(txn, &a3_fork_other_author).is_err(),
            "Cross-author prev_action collision should return an error"
        );
    });
}
