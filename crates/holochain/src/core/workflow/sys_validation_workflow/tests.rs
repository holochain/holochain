use super::*;
use crate::retry_until_timeout;
use crate::sweettest::*;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::{assert_limbo_empty, wait_for_integration};
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain_wasm_test_utils::TestWasm;
use std::convert::TryFrom;
use std::time::Duration;
use {
    crate::core::workflow::sys_validation_workflow::types::Outcome, ::fixt::fixt,
    holochain_zome_types::fixt::EntryFixturator, std::convert::TryInto,
};
// The ops constructed by hand in this module are built from legacy per-variant
// `Action` fixtures (`fixt!(Create)`, `fixt!(Dna)`, ...) and projected to v2 via
// `from_legacy_action`, so `Action` (otherwise the v2 struct via the ambient
// preludes) is pinned to the legacy shape here.
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;
use holochain_zome_types::dht_v2::SignedAction;

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
    let op: DhtOp = ChainOp::CreateEntry(
        SignedAction::new(
            from_legacy_action(&Action::Create(mismatched_action)),
            fixt!(Signature),
        ),
        OpEntry::Present(fixt!(Entry)),
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

    // Inject the invalid op directly into bob's DHT store
    let op = DhtOpHashed::from_content_sync(op);
    conductor
        .spaces
        .dht_store(dna.dna_hash())
        .unwrap()
        .record_incoming_ops(vec![(op, false)])
        .await
        .unwrap();

    //- Trigger sys validation
    conductor
        .get_cell_triggers(alice.cells()[0].cell_id())
        .await
        .unwrap()
        .sys_validation
        .trigger(&"test");

    let warrant_author = alice.agent().clone();
    retry_fn_until_timeout(
        || async {
            let num_of_warrants = conductor
                .spaces
                .dht_store(dna.dna_hash())
                .unwrap()
                .as_read()
                .warrants_by_author(warrant_author.clone())
                .await
                .unwrap()
                .len();
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
    let signed_dna_action = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&dna_action)),
    )
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

    let signed_original = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&original_action)),
    )
    .await
    .unwrap();
    let signed_forked = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&forked_action)),
    )
    .await
    .unwrap();

    let original_action_hash = signed_original.as_hash().clone();
    let forked_action_hash = signed_forked.as_hash().clone();
    let expected_seq = 1u32;

    // Build ChainOps for genesis, original, and forked actions directly from
    // the v2 signed actions (the hash carried on each `SignedActionHashed` is
    // the same content-derived v2 identity the op hash is built from).
    let (dna_hashed, dna_sig) = signed_dna_action.into_inner();
    let prev_op = ChainOp::CreateRecord(
        SignedAction::new(dna_hashed.into_content(), dna_sig),
        OpEntry::ActionOnly,
    );

    let (orig_hashed, orig_sig) = signed_original.into_inner();
    let original_op = ChainOp::CreateRecord(
        SignedAction::new(orig_hashed.into_content(), orig_sig),
        OpEntry::Present(original_entry),
    );

    let (fork_hashed, fork_sig) = signed_forked.into_inner();
    let forked_op = ChainOp::CreateRecord(
        SignedAction::new(fork_hashed.into_content(), fork_sig),
        OpEntry::Present(forked_entry),
    );

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
    // forked op (as pending validation) into Bob's DHT store
    let prev_op_hashed = DhtOpHashed::from_content_sync(prev_op);
    let original_op_hashed = DhtOpHashed::from_content_sync(original_op);
    let forked_op_hashed = DhtOpHashed::from_content_sync(forked_op);
    conductor
        .spaces
        .dht_store(dna.dna_hash())
        .unwrap()
        .record_incoming_ops(vec![
            (prev_op_hashed, false),
            (original_op_hashed, false),
            (forked_op_hashed, false),
        ])
        .await
        .unwrap();

    // Check that Bob authored a chain fork warrant with the correct action hashes
    retry_until_timeout!(60_000, 500, {
        // Trigger sys validation
        conductor
            .get_cell_triggers(&bob_cell_id)
            .await
            .unwrap()
            .sys_validation
            .trigger(&"test");

        let warrants: Vec<WarrantOp> = conductor
            .spaces
            .dht_store(dna.dna_hash())
            .unwrap()
            .as_read()
            .warrants_by_author(bob_pubkey.clone())
            .await
            .unwrap();

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
    let signed_dna_action = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&dna_action)),
    )
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

    let signed_action1 = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&action1)),
    )
    .await
    .unwrap();
    let signed_action2 = SignedActionHashed::sign(
        &keystore,
        holo_hash::HoloHashed::from_content_sync(from_legacy_action(&action2)),
    )
    .await
    .unwrap();

    let action1_hash = signed_action1.as_hash().clone();
    let action2_hash = signed_action2.as_hash().clone();

    // Create ChainOps for the previous action and both forked actions,
    // directly from the v2 signed actions (the hash carried on each
    // `SignedActionHashed` is the same content-derived v2 identity the op
    // hash is built from).
    let (dna_hashed, dna_sig) = signed_dna_action.into_inner();
    let prev_op = ChainOp::CreateRecord(
        SignedAction::new(dna_hashed.into_content(), dna_sig),
        OpEntry::ActionOnly,
    );

    let (action1_hashed, sig1) = signed_action1.into_inner();
    let op1 = ChainOp::CreateRecord(
        SignedAction::new(action1_hashed.into_content(), sig1),
        OpEntry::Present(entry1),
    );

    let (action2_hashed, sig2) = signed_action2.into_inner();
    let op2 = ChainOp::CreateRecord(
        SignedAction::new(action2_hashed.into_content(), sig2),
        OpEntry::Present(entry2),
    );

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

    // Inject the previous action and both forked ops into Bob's DHT store
    let prev_op_hashed = DhtOpHashed::from_content_sync(prev_op);
    let op1_hashed = DhtOpHashed::from_content_sync(op1);
    let op2_hashed = DhtOpHashed::from_content_sync(op2);
    conductor
        .spaces
        .dht_store(dna.dna_hash())
        .unwrap()
        .record_incoming_ops(vec![
            (prev_op_hashed, false),
            (op1_hashed, false),
            (op2_hashed, false),
        ])
        .await
        .unwrap();

    // Check that Bob authored 2 chain fork warrants
    retry_until_timeout!(60_000, 500, {
        // Trigger sys validation
        conductor
            .get_cell_triggers(&bob_cell_id)
            .await
            .unwrap()
            .sys_validation
            .trigger(&"test");

        let warrants: Vec<WarrantOp> = conductor
            .spaces
            .dht_store(dna.dna_hash())
            .unwrap()
            .as_read()
            .warrants_by_author(bob_pubkey.clone())
            .await
            .unwrap();

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
    // Assert against the new `holochain_data` DHT store, which is the
    // authoritative source for integration during the migration; the legacy
    // `DhtOp` table is now a downstream mirror. Poll every 100 ms for up to
    // 10 seconds, exiting early once the expected ops are integrated.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    let dht_store = conductors[0]
        .spaces
        .dht_store(alice_cell_id.dna_hash())
        .unwrap();

    bob_links_in_a_legit_way(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // 9 ops from the three authored records plus 14 genesis ops (both agents).
    // Init is not run because we aren't calling the zome.
    let expected_count: u64 = 9 + 14;

    wait_for_integration(&dht_store, expected_count, num_attempts, delay_per_attempt).await;
    assert_limbo_empty(&dht_store).await;

    // Authors an op with an oversized link tag, which is rejected and produces
    // an InvalidChainOp warrant.
    bob_makes_a_large_link(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // 14 chain ops + 1 warrant op on top of the running tally. The rejected ops
    // are still integrated (with a rejected status), so they count here too.
    let expected_count = 14 + 1 + expected_count;

    wait_for_integration(&dht_store, expected_count, num_attempts, delay_per_attempt).await;
    assert_limbo_empty(&dht_store).await;
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
