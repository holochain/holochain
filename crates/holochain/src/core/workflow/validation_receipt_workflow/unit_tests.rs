use crate::core::queue_consumer::WorkComplete;
use crate::core::workflow::validation_receipt_workflow::validation_receipt_workflow;
use crate::prelude::CreateFixturator;
use crate::prelude::SignatureFixturator;
use ::fixt::fixt;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::HasHash;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::*;
use holochain_state::test_utils::test_dht_store;
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn no_running_cells() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipts().never(); // Verify no receipts sent
    let dna = Arc::new(dna);

    let dna_hash = fixt!(DnaHash);

    let dht_store = test_dht_store(dna_hash.clone()).await;
    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store,
        dna,
        keystore,
        vec![].into_iter().collect(), // No running cells
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
}

#[tokio::test(flavor = "multi_thread")]
async fn do_not_block_or_send_to_self() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let author = fixt!(AgentPubKey);

    let dht_store = test_dht_store(dna_hash.clone()).await;

    // Create a valid op that would require a validation receipt except that it's created by us
    let (_, valid_op_hash) =
        create_op_with_status(&dht_store, Some(author.clone()), ValidationStatus::Valid)
            .await
            .unwrap();

    // Create a rejected op which would usually cause a block but it's created by us
    let (_, rejected_op_hash) =
        create_op_with_status(&dht_store, Some(author.clone()), ValidationStatus::Rejected)
            .await
            .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_send_validation_receipts().never(); // Verify no receipts sent
    let dna = Arc::new(dna);

    let validator = CellId::new(dna_hash.clone(), author);

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(), // No running cells
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&valid_op_hash)
        .await
        .unwrap());
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&rejected_op_hash)
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn block_invalid_op_author() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let dht_store = test_dht_store(dna_hash.clone()).await;

    // Any op created by somebody else, which has been rejected by validation.
    let (_author, op_hash) = create_op_with_status(&dht_store, None, ValidationStatus::Rejected)
        .await
        .unwrap();

    // We'll still send a validation receipt, but we should also block them
    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_was_agent_recently_online()
        .return_once(|_| Ok(true));
    dna.expect_send_validation_receipts()
        .return_once(|_, _| Ok(()));
    let dna = Arc::new(dna);

    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    // The op was rejected, but the `require_receipt` flag should still be cleared
    // so we don't reprocess the op.
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash)
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn continues_if_receipt_cannot_be_signed() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let dht_store = test_dht_store(dna_hash.clone()).await;

    // Any op created by somebody else, which is valid
    let (_, op_hash) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_was_agent_recently_online()
        .return_once(|_| Ok(true));
    dna.expect_send_validation_receipts().never();
    let dna = Arc::new(dna);

    let invalid_validator = CellId::new(
        dna_hash.clone(),
        fixt!(AgentPubKey), // Not valid because it won't be found in Lair
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![invalid_validator].into_iter().collect(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash)
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn send_validation_receipt() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let dht_store = test_dht_store(dna_hash.clone()).await;

    // Any op created by somebody else, which is valid
    let (_, op_hash) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_was_agent_recently_online()
        .return_once(|_| Ok(true));
    dna.expect_send_validation_receipts()
        .return_once(|_, _| Ok(()));
    let dna = Arc::new(dna);

    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(), // No running cells
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    // Should no longer require a receipt
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash)
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_for_some_ops_does_not_prevent_the_workflow_proceeding() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let dht_store = test_dht_store(dna_hash.clone()).await;

    let (author1, op_hash1) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let (author2, op_hash2) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let mut dna = MockHolochainP2pDnaT::new();
    dna.expect_was_agent_recently_online()
        .returning(|_| Ok(true));
    // Both authors are processed; the order is not guaranteed by the DB query.
    // Author1's send returns an error; author2's send succeeds.
    dna.expect_send_validation_receipts()
        .times(1)
        .withf(move |author: &AgentPubKey, _| *author == author1)
        .returning(|_, _| Err("I'm a test error".into()));

    dna.expect_send_validation_receipts()
        .times(1)
        .withf(move |author: &AgentPubKey, _| *author == author2)
        .returning(|_, _| Ok(()));
    let dna = Arc::new(dna);

    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(), // No running cells
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    // Sending the receipt to this author returned an error,
    // so we did NOT clear the wants receipt flag.
    assert!(dht_store
        .as_read()
        .op_requires_receipt(&op_hash1)
        .await
        .unwrap());

    // But even after we got the above error, we proceeded to
    // send the receipt for the second author which DID work,
    // so its flag is cleared.
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash2)
        .await
        .unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn skips_authors_not_recently_online_and_clears_require_receipt() {
    holochain_trace::test_run();

    let keystore = holochain_keystore::test_keystore();

    let dna_hash = fixt!(DnaHash);
    let dht_store = test_dht_store(dna_hash.clone()).await;

    // Create ops from two different authors
    let (author1, op_hash1) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let (author2, op_hash2) = create_op_with_status(&dht_store, None, ValidationStatus::Valid)
        .await
        .unwrap();

    let author1_clone = author1.clone();
    let mut dna = MockHolochainP2pDnaT::new();

    // Author1 is not recently online, author2 is
    dna.expect_was_agent_recently_online()
        .times(2)
        .returning(move |agent| Ok(agent != author1_clone));

    // Author1 was not recently online, so no receipts should be sent to them
    let author1_clone2 = author1.clone();
    dna.expect_send_validation_receipts()
        .never()
        .withf(move |author: &AgentPubKey, _| *author == author1_clone2);

    // Author2 was recently online, so receipts should be sent
    dna.expect_send_validation_receipts()
        .times(1)
        .withf(move |author: &AgentPubKey, _| *author == author2)
        .returning(|_, _| Ok(()));

    let dna = Arc::new(dna);

    let validator = CellId::new(
        dna_hash.clone(),
        keystore.new_sign_keypair_random().await.unwrap(),
    );

    let work_complete = validation_receipt_workflow(
        Arc::new(dna_hash),
        dht_store.clone(),
        dna,
        keystore,
        vec![validator].into_iter().collect(),
    )
    .await
    .unwrap();

    assert_eq!(WorkComplete::Complete, work_complete);

    // Author1 was not recently online, so require_receipt should be cleared
    // without attempting to send. A new publish will re-set it.
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash1)
        .await
        .unwrap());

    // Author2 was online and sending succeeded, so require_receipt is also cleared.
    assert!(!dht_store
        .as_read()
        .op_requires_receipt(&op_hash2)
        .await
        .unwrap());
}

async fn create_op_with_status(
    dht_store: &DhtStore,
    author: Option<AgentPubKey>,
    validation_status: ValidationStatus,
) -> StateMutationResult<(AgentPubKey, DhtOpHash)> {
    use holochain_state::dht_store::{AppOutcome, SysOutcome};

    // The actual op does not matter, just some of the status fields
    let mut create_action = fixt!(Create);
    let author = author.unwrap_or_else(|| fixt!(AgentPubKey));
    create_action.author = author.clone();
    let action = Action::Create(create_action);

    let v2_action = holochain_types::dht_v2::from_legacy_action(&action);
    let signed = holochain_types::dht_v2::SignedAction::new(v2_action, fixt!(Signature));
    let op = holochain_types::dht_v2::DhtOpHashed::from_content_sync(
        holochain_types::dht_v2::DhtOp::from(holochain_types::dht_v2::ChainOp::AgentActivity(
            signed,
        )),
    );

    let test_op_hash = op.as_hash().clone();

    // Write the op through the full validation + integration pipeline so that
    // DhtStore::pending_validation_receipts sees it in integrated +
    // require_receipt state. The hash is derived from the same op content, so
    // test_op_hash matches.
    dht_store
        .record_incoming_ops(vec![(op, true)])
        .await
        .unwrap();

    let sys_outcome = match validation_status {
        ValidationStatus::Valid => SysOutcome::Accepted,
        _ => SysOutcome::Rejected,
    };
    dht_store
        .record_chain_op_sys_validation_outcomes(vec![(test_op_hash.clone(), sys_outcome)])
        .await
        .unwrap();

    let app_outcome = match validation_status {
        ValidationStatus::Valid => AppOutcome::Accepted,
        _ => AppOutcome::Rejected,
    };
    dht_store
        .record_app_validation_outcomes(vec![(test_op_hash.clone(), app_outcome)])
        .await
        .unwrap();

    dht_store
        .integrate_ready_ops(Timestamp::now())
        .await
        .unwrap();
    // record_incoming_ops sets require_receipt = true, matching the legacy fixture.

    Ok((author, test_op_hash))
}
