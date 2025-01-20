//! Sys validation tests
//!
//! TESTED:
//! - Mismatched signatures are rejected
//! - Any action other than DNA cannot be at seq 0
//! - The DNA action can only be validated if the chain is empty,
//!     and its timestamp must not be less than the origin time
//!     (this "if chain not empty" thing is a bit weird,
//!     TODO refactor to not look in the db)
//! - Timestamps must increase monotonically
//! - Sequence numbers must increment by 1 for each new action
//! - Entry type in the action matches the entry variant
//! - Hash integrity check. The hash of an entry always matches what's in the action.
//! - The size of an entry does not exceed the max.
//! - Check that updates can't switch the entry type
//! - The link tag size is bounded
//! - Check the AppEntryDef is valid for the zome and the EntryDefId and ZomeIndex are in range.
//! - Check that StoreEntry never contains a private entry type
//! - Test that a given sequence of actions constitutes a valid chain w.r.t. its backlinks
//!
//! TO TEST:
//! - Create and Update Agent can only be preceded by AgentValidationPkg
//! - Author must match the entry hash of the most recent Create/Update Agent
//! - Genesis must be correct:
//!     - Explicitly check action seqs 0, 1, and 2.
//! - There can only be one InitZomesCompleted
//! - All backlinks are in-chain (prev action, etc.)
//!

use super::*;
use crate::conductor::space::TestSpaces;
use crate::core::workflow::sys_validation_workflow::sys_validate_record;
use crate::sweettest::SweetAgents;
use crate::sweettest::SweetConductor;
use ::fixt::prelude::*;
use error::SysValidationError;
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holochain_cascade::MockCascade;
use holochain_keystore::test_keystore;
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_types::test_utils::valid_arbitrary_chain;
use holochain_types::test_utils::ActionRefMut;
use holochain_zome_types::Action;
use matches::assert_matches;
use std::time::Duration;

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
    let mut ec = Create {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now(),
        action_seq: 6,
        prev_action: fixt!(ActionHash),
        entry_type: EntryType::AgentPubKey,
        entry_hash: fixt!(EntryHash),
        weight: EntryRateWeight::default(),
    };
    let entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let hash = EntryHash::with_data_sync(&entry);
    let action: Action = ec.clone().into();

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

    ec.entry_hash = hash;
    let action: Action = ec.clone().into();

    let eh = action.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(check_entry_hash(eh, &entry), Ok(()));
    assert_matches!(
        check_new_entry_action(&Action::CreateLink(CreateLink {
            author: fixt!(AgentPubKey),
            timestamp: Timestamp::now(),
            action_seq: 8,
            prev_action: fixt!(ActionHash),
            base_address: fixt!(EntryHash).into(),
            target_address: fixt!(EntryHash).into(),
            zome_index: 0.into(),
            link_type: LinkType::new(3),
            tag: ().into(),
            weight: RateWeight::default(),
        })),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::NotNewEntry(_)
        ))
    );
}

/// Check that StoreEntry does not have a private entry type
#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_filters_private_entry() {
    let dna = fixt!(DnaHash);
    let spaces = TestSpaces::new([dna.clone()]).await;
    let space = Arc::new(spaces.test_spaces[&dna].space.clone());
    let vault = space.dht_db.clone();
    let keystore = test_keystore();
    let (tx, _rx) = TriggerSender::new();

    let private_entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        vec![1, 3, 5],
    ))));
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let app_entry_def = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    let create = Create {
        author: author.clone(),
        timestamp: Timestamp::now(),
        action_seq: 5,
        prev_action: fixt!(ActionHash),
        entry_type: EntryType::App(app_entry_def),
        entry_hash: EntryHash::with_data_sync(&private_entry),
        weight: EntryRateWeight::default(),
    };
    let action = Action::Create(create);
    let signature = author.sign(&keystore, &action).await.unwrap();

    let shh =
        SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), signature);
    let record = Record::new(shh, Some(private_entry));

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_entry(record.clone()).await.unwrap();
    let num_ops: usize = vault
        .read_async(move |txn| -> DatabaseResult<usize> {
            Ok(txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(num_ops, 0);

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_record(record.clone()).await.unwrap();
    let num_ops: usize = vault
        .read_async(move |txn| -> DatabaseResult<usize> {
            Ok(txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(num_ops, 1);
    let num_entries: usize = vault
        .read_async(move |txn| -> DatabaseResult<usize> {
            Ok(txn.query_row("SELECT COUNT(rowid) FROM Entry", [], |row| row.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(num_entries, 0);
}

/// Test that the valid_chain contrafact matches our chain validation function,
/// since many other tests will depend on this constraint
#[tokio::test(flavor = "multi_thread")]
// XXX: the valid_arbitrary_chain as used here can't handle actions with
// sys validation dependencies, so we filter out those action types.
// Also, there are several other problems here that need to be addressed
// to make this not flaky.
#[ignore = "flaky"]
async fn valid_chain_fact_test() {
    let n = 100;
    let keystore = SweetConductor::from_standard_config().await.keystore();
    let author = SweetAgents::one(keystore.clone()).await;

    let mut chain = valid_arbitrary_chain(&keystore, author, n).await;

    validate_chain(chain.iter().map(|r| r.signed_action()), &None).unwrap();

    let mut last = chain.pop().unwrap();
    let penult = chain.last().unwrap();

    // clean up this record so it's valid
    *last.as_action_mut().timestamp_mut() =
        (penult.action().timestamp() + Duration::from_secs(1)).unwrap();
    // re-sign it
    last.signed_action = SignedActionHashed::sign(
        &keystore,
        ActionHashed::from_content_sync(last.action().clone()),
    )
    .await
    .unwrap();

    let cascade = MockCascade::with_records(chain);

    sys_validate_record(&last, Arc::new(cascade)).await.unwrap();
}
