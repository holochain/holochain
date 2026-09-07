//! Sys validation tests
//!
//! TESTED:
//! - Mismatched signatures are rejected
//! - Any action other than DNA cannot be at seq 0
//! - The DNA action can only be validated if the chain is empty,
//!   and its timestamp must not be less than the origin time
//! - Timestamps must increase monotonically
//! - Sequence numbers must increment by 1 for each new action
//! - Entry type in the action matches the entry variant
//! - Hash integrity check. The hash of an entry always matches what's in the action.
//! - The size of an entry does not exceed the max.
//! - Check that updates can't switch the entry type
//! - The link tag size is bounded
//! - Check the AppEntryDef is valid for the zome and the EntryDefId and ZomeIndex are in range.
//! - Check that CreateEntry never contains a private entry type
//! - Test that a given sequence of actions constitutes a valid chain w.r.t. its backlinks
//!
// TODO Add tests for:
// - Create and Update Agent can only be preceded by AgentValidationPkg
// - Author must match the entry hash of the most recent Create/Update Agent
// - Genesis must be correct:
//     - Explicitly check action seqs 0, 1, and 2.
// - There can only be one InitZomesCompleted
// - All backlinks are in-chain (prev action, etc.)
//
// TODO This "The DNA action can only be validated if the chain is empty" thing is a bit weird,
//  refactor to not look in the db

use super::*;
use crate::conductor::space::TestSpaces;
use crate::core::workflow::sys_validation_workflow::counterfeit_check_action;
use ::fixt::prelude::*;
use error::SysValidationError;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holochain_keystore::test_keystore;
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::fixt::{
    ActionFixturator, CloseChainAction, CreateAction, CreateLinkAction,
};
use matches::assert_matches;

/// Entry type in the action matches the entry variant
#[test]
fn check_entry_type_test() {
    let entry_fixt = EntryFixturator::new(Predictable);
    let et_fixt = EntryTypeFixturator::new(Predictable);

    for (e, et) in entry_fixt.zip(et_fixt).take(4) {
        assert_matches!(check_entry_type(&et, &e), Ok(()));
    }

    // Offset by 1
    let entry_fixt = EntryFixturator::new(Predictable);
    let mut et_fixt = EntryTypeFixturator::new(Predictable);
    et_fixt.next().unwrap();

    for (e, et) in entry_fixt.zip(et_fixt).take(4) {
        assert_matches!(
            check_entry_type(&et, &e),
            Err(SysValidationError::ValidationOutcome(
                ValidationOutcome::EntryTypeMismatch
            ))
        );
    }
}

/// Hash integrity check. The hash of an entry always matches what's in the action.
#[test]
fn check_entry_hash_test() {
    let mut ec = fixt!(Action, CreateAction);
    *ec.entry_type_mut().unwrap() = EntryType::AgentPubKey;
    *ec.entry_hash_mut().unwrap() = fixt!(EntryHash);
    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let hash = EntryHash::with_data_sync(&entry);
    let action = ec.clone();

    // First check it should have an entry
    assert_matches!(check_new_entry_action(&action), Ok(()));
    // Safe to unwrap if new entry
    let eh = action.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(
        check_entry_hash(eh, &entry),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryHash
        ))
    );

    *ec.entry_hash_mut().unwrap() = hash;
    let action = ec.clone();

    let eh = action.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(check_entry_hash(eh, &entry), Ok(()));
    let create_link = fixt!(Action, CreateLinkAction);
    assert_matches!(
        check_new_entry_action(&create_link),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::NotNewEntry(_)
        ))
    );
}

/// Check that CreateEntry does not have a private entry type
#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_filters_private_entry() {
    let dna = fixt!(DnaHash);
    let spaces = TestSpaces::new([dna.clone()]).await;
    let space = Arc::new(spaces.test_spaces[&dna].space.clone());
    let keystore = test_keystore();
    let (tx, _rx) = TriggerSender::new();

    let private_entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let app_entry_def = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    let mut action = fixt!(Action, CreateAction);
    action.header.author = author.clone();
    action.header.timestamp = Timestamp::now();
    action.header.action_seq = 5;
    action.header.prev_action = Some(fixt!(ActionHash));
    *action.entry_type_mut().unwrap() = EntryType::App(app_entry_def);
    *action.entry_hash_mut().unwrap() = EntryHash::with_data_sync(&private_entry);
    let signature = author.sign(&keystore, &action).await.unwrap();

    let shh = SignedActionHashed::with_presigned(
        holo_hash::HoloHashed::from_content_sync(action),
        signature,
    );
    let record = Record::new(shh, RecordEntry::Present(private_entry));

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_entry(record.clone()).await.unwrap();
    let num_ops = space.dht_store.as_read().count_all_ops().await.unwrap();
    assert_eq!(num_ops, 0);

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_record(record.clone()).await.unwrap();
    let num_ops = space.dht_store.as_read().count_all_ops().await.unwrap();
    assert_eq!(num_ops, 1);
    let num_entries = space.dht_store.as_read().count_entries().await.unwrap();
    assert_eq!(num_entries, 0);
}

/// Every action, including a `CloseChain` that names an agent migration
/// target, must be signed by its author. A `CloseChain` signed by the target
/// key is a forgery: it would let any agent close, and so fork, another
/// agent's chain in that agent's name (#5981).
#[tokio::test(flavor = "multi_thread")]
async fn close_chain_signed_by_migration_target_is_counterfeit() {
    let keystore = test_keystore();
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let forger = keystore.new_sign_keypair_random().await.unwrap();

    let mut action = fixt!(Action, CloseChainAction);
    action.header.author = author.clone();
    let ActionData::CloseChain(close) = &mut action.data else {
        panic!("the CloseChainAction curve must produce a CloseChain");
    };
    close.new_target = Some(MigrationTarget::Agent(forger.clone()));

    // Signed by the forger, who is also the migration target: rejected.
    let forged_sig = forger.sign(&keystore, &action).await.unwrap();
    assert_matches!(
        counterfeit_check_action(&forged_sig, &action).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::CounterfeitAction(..)
        ))
    );

    // Signed by the author: accepted.
    let sig = author.sign(&keystore, &action).await.unwrap();
    counterfeit_check_action(&sig, &action).await.unwrap();
}

/// A `CreateEntry` op is the public entry-authority op, so it must never carry
/// a private entry — even with the entry body withheld. A peer that crafts one
/// is attempting to announce a private entry's existence to the entry
/// authority, so sys validation rejects it as a leak. The guard is
/// `CreateEntry`-specific: a private entry is legitimate on a `CreateRecord`
/// op, where a withheld body is expected.
#[test]
fn create_entry_op_rejects_private_entry() {
    let mut private_create = fixt!(Action, CreateAction);
    *private_create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef::new(
        0.into(),
        0.into(),
        EntryVisibility::Private,
    ));
    let sa = SignedAction::new(private_create, Signature::from([0u8; 64]));

    for withheld in [OpEntry::Hidden, OpEntry::ActionOnly] {
        assert_matches!(
            check_entry_visibility(&ChainOp::CreateEntry(sa.clone(), withheld)),
            Err(SysValidationError::ValidationOutcome(
                ValidationOutcome::PrivateEntryLeaked
            ))
        );
    }

    assert_matches!(
        check_entry_visibility(&ChainOp::CreateRecord(sa, OpEntry::Hidden)),
        Ok(())
    );
}
