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
use crate::test_utils::fake_genesis_for_agent;
use crate::test_utils::rebuild_record;
use crate::test_utils::sign_record;
use crate::test_utils::valid_arbitrary_chain;
use ::fixt::prelude::*;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use contrafact::Fact;
use error::SysValidationError;

use futures::FutureExt;
use holochain_cascade::MockCascade;
use holochain_keystore::test_keystore;
use holochain_keystore::AgentPubKeyExt;
use holochain_keystore::MetaLairClient;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::prelude::DatabaseResult;
use holochain_state::prelude::test_authored_db;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::test_utils::chain::{TestChainHash, TestChainItem};
use holochain_zome_types::facts::ActionRefMut;
use holochain_zome_types::Action;
use matches::assert_matches;
use std::time::Duration;

fn matching_record(g: &mut Unstructured, f: impl Fn(&Record) -> bool) -> Record {
    // get anything but a Dna
    let mut dep = Record::arbitrary(g).unwrap();
    while !f(&dep) {
        dep = Record::arbitrary(g).unwrap();
    }
    dep
}

fn is_dna_record(r: &Record) -> bool {
    matches!(r.action(), Action::Dna(_))
}
fn is_pkg_record(r: &Record) -> bool {
    matches!(r.action(), Action::AgentValidationPkg(_))
}
fn is_entry_record(r: &Record) -> bool {
    matches!(r.action(), Action::Create(_) | Action::Update(_))
        && r.entry
            .as_option()
            .map(|e| entry_type_matches(r.action().entry_type().unwrap(), e))
            .unwrap_or(false)
}

async fn record_with_deps(keystore: &MetaLairClient, action: Action) -> (Record, Vec<Record>) {
    record_with_deps_fixup(keystore, action, true).await
}

/// Creates a valid Record using the given action and constructs a MockCascade
/// with the minimal set of records to satisfy the validation dependencies
/// for that Record. Updates the input action's backlinks to point to these other
/// injected actions, and returns it along with the created MockCascade.
async fn record_with_deps_fixup(
    keystore: &MetaLairClient,
    mut action: Action,
    fixup: bool,
) -> (Record, Vec<Record>) {
    let mut g = random_generator();

    if fixup {
        if !matches!(action, Action::Dna(_)) && action.action_seq() < 2 {
            // In case this is an Agent entry, allow the previous to be something other than Dna
            *action.action_seq_mut().unwrap() = 2;
        }

        *action.author_mut() = fake_agent_pubkey_1();
    }

    let (entry, deps) = match &mut action {
        Action::Dna(_) => (None, vec![]),
        action => {
            let mut deps = vec![];
            let prev_seq = action.action_seq() - 1;

            let entry = action.entry_data_mut().map(|(entry_hash, entry_type)| {
                let et = entry_type.clone();
                let entry = contrafact::brute("matching entry", move |e: &Entry| matches!((&et, e), (EntryType::AgentPubKey, Entry::Agent(_)) | (EntryType::App(_), Entry::App(_)) | (EntryType::CapClaim, Entry::CapClaim(_)) | (EntryType::CapGrant, Entry::CapGrant(_))))
                .build(&mut g);

                *entry_hash = EntryHash::with_data_sync(&entry);

                entry
            });

            let mut prev = if prev_seq == 0 {
                matching_record(&mut g, is_dna_record)
            } else {
                let mut prev = match action {
                    Action::Create(Create {
                        entry_type: EntryType::AgentPubKey,
                        ..
                    })
                    | Action::Update(Update {
                        entry_type: EntryType::AgentPubKey,
                        ..
                    }) => matching_record(&mut g, is_pkg_record),
                    _ => matching_record(&mut g, |r| !is_dna_record(r) && !is_pkg_record(r)),
                };
                *prev.as_action_mut().action_seq_mut().unwrap() = action.action_seq() - 1;
                prev
            };
            *prev.as_action_mut().author_mut() = action.author().clone();
            *prev.as_action_mut().timestamp_mut() =
                (action.timestamp() - Duration::from_micros(1)).unwrap();
            // NOTE: hash integrity is broken at this point, record needs to be rebuilt
            let prev_hash = prev.action_address().clone();
            *action.prev_action_mut().unwrap() = prev_hash.clone();
            assert_eq!(*action.prev_action().unwrap(), prev_hash);
            deps.push(prev);

            match action {
                Action::Create(_create) => {}
                Action::Update(update) => {
                    let mut create = matching_record(&mut g, |r| {
                        // Updates to agent pub key can get tricky
                        r.action().entry_type() != Some(&EntryType::AgentPubKey)
                            && is_entry_record(r)
                    });
                    update.original_action_address = create.action_address().clone();
                    update.original_entry_address =
                        create.entry().as_option().unwrap().to_hash().clone();
                    *create.as_action_mut().entry_data_mut().unwrap().0 =
                        create.entry().as_option().unwrap().to_hash().clone();
                    *create.as_action_mut().entry_data_mut().unwrap().1 = update.entry_type.clone();
                    deps.push(create);
                }
                Action::Delete(delete) => {
                    let dep = matching_record(&mut g, is_entry_record);
                    delete.deletes_address = dep.action_address().clone();
                    delete.deletes_entry_address =
                        dep.entry().as_option().unwrap().to_hash().clone();

                    deps.push(dep);
                }
                Action::CreateLink(link) => {
                    let base = Record::arbitrary(&mut g).unwrap();
                    let target = Record::arbitrary(&mut g).unwrap();
                    link.base_address = base.action_address().clone().into();
                    link.target_address = target.action_address().clone().into();
                    deps.push(base);
                    deps.push(target);
                }
                Action::DeleteLink(delete) => {
                    let base = Record::arbitrary(&mut g).unwrap();
                    let create =
                        matching_record(&mut g, |r| matches!(r.action(), Action::CreateLink(_)));
                    delete.base_address = base.action_address().clone().into();
                    delete.link_add_address = create.action_address().clone();
                    deps.push(base);
                    deps.push(create);
                }
                Action::AgentValidationPkg(_)
                | Action::CloseChain(_)
                | Action::InitZomesComplete(_)
                | Action::OpenChain(_) => {
                    // no new deps needed to make this valid
                }
                Action::Dna(_) => unreachable!(),
            };

            (entry, deps)
        }
    };

    assert_eq!(*action.author(), fake_agent_pubkey_1());

    let record = sign_record(keystore, action, entry).await;

    (record, deps)
}

async fn record_with_cascade(
    keystore: &MetaLairClient,
    action: Action,
) -> (Record, Arc<MockCascade>) {
    let (record, deps) = record_with_deps(keystore, action).await;
    (record, Arc::new(MockCascade::with_records(deps)))
}

async fn assert_valid_action(keystore: &MetaLairClient, action: Action) {
    let (record, deps) = record_with_deps(keystore, action).await;
    let cascade = Arc::new(MockCascade::with_records(deps.clone()));
    let result = sys_validate_record(&record, cascade).await;
    if result.is_err() {
        dbg!(&deps, &record);
        result.unwrap();
    }
}

/// Mismatched signatures are rejected
#[tokio::test(flavor = "multi_thread")]
async fn test_record_with_cascade() {
    let mut g = random_generator();

    let keystore = holochain_keystore::test_keystore();
    for _ in 0..100 {
        let op =
            holochain_types::facts::valid_chain_op(keystore.clone(), fake_agent_pubkey_1(), false)
                .build(&mut g);
        let action = op.action().clone();
        assert_valid_action(&keystore, action).await;
    }
}

/// Mismatched signatures are rejected
#[tokio::test(flavor = "multi_thread")]
async fn verify_action_signature_test() {
    let mut g = random_generator();

    let keystore = holochain_keystore::test_keystore();
    let action = CreateLink::arbitrary(&mut g).unwrap();
    let (record_valid, cascade) = record_with_cascade(&keystore, Action::CreateLink(action)).await;

    let wrong_signature = Signature([1_u8; 64]);
    let action_invalid =
        SignedActionHashed::new_unchecked(record_valid.action().clone(), wrong_signature);
    let record_invalid = Record::new(action_invalid, None);

    sys_validate_record(&record_valid, cascade.clone())
        .await
        .unwrap();
    sys_validate_record(&record_invalid, cascade)
        .await
        .unwrap_err();
}

/// Any action other than DNA cannot be at seq 0
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_action() {
    let mut g = random_generator();

    let keystore = holochain_keystore::test_keystore();
    let mut action = Action::Delete(Delete::arbitrary(&mut g).unwrap());
    *action.author_mut() = keystore.new_sign_keypair_random().await.unwrap();

    *action.action_seq_mut().unwrap() = 7;

    assert_valid_action(&keystore, action.clone()).await;

    *action.action_seq_mut().unwrap() = 0;

    // This check is manual because `validate_action` will modify any action
    // coming in with a 0 action_seq since it knows that can't be valid.
    {
        let mut cascade = MockCascade::new();
        cascade
            .expect_retrieve_action()
            .times(2)
            // Doesn't matter what we return, the action should be rejected before deps are checked.
            .returning(|_, _| async move { Ok(None) }.boxed());

        let actual = sys_validate_record(
            &sign_record(&keystore, action, None).await,
            Arc::new(cascade),
        )
        .await
        .unwrap_err()
        .into_outcome();

        let actual = match actual {
            Some(ValidationOutcome::PrevActionError(pae)) => pae.source,
            v => panic!("Expected PrevActionError, got {:?}", v),
        };
        assert_eq!(actual, PrevActionErrorKind::InvalidRoot);
    }

    // Dna is always ok because of the type system
    let action = Action::Dna(Dna::arbitrary(&mut g).unwrap());
    assert_valid_action(&keystore, action.clone()).await;
}

/// The DNA action can only be validated if the chain is empty,
/// and its timestamp must not be less than the origin time
/// (this "if chain not empty" thing is a bit weird, TODO refactor to not look in the db)
#[tokio::test(flavor = "multi_thread")]
async fn check_valid_if_dna_test() {
    let mut g = random_generator();

    let tmp = test_authored_db();
    let tmp_dht = test_dht_db();
    let tmp_cache = test_cache_db();
    let keystore = test_keystore();
    let db = tmp.to_db();
    // Test data
    let _activity_return = [ActionHash::arbitrary(&mut g).unwrap()];

    let mut dna_def = DnaDef::arbitrary(&mut g).unwrap();
    dna_def.modifiers.origin_time = Timestamp::MIN;

    // Empty store not dna
    let action = CreateLink::arbitrary(&mut g).unwrap();
    let cache: DhtDbQueryCache = tmp_dht.to_db().into();
    let mut workspace = SysValidationWorkspace::new(
        db.clone().into(),
        tmp_dht.to_db(),
        cache.clone(),
        tmp_cache.to_db(),
        Arc::new(dna_def.clone()),
        std::time::Duration::from_secs(10),
    );

    // Initializing the cache actually matters. TODO: why?
    cache.get_state().await;

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()),
        Ok(())
    );
    let mut action = Dna::arbitrary(&mut g).unwrap();
    action.hash = DnaHash::with_data_sync(&dna_def);

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()),
        Ok(())
    );

    // - Test that an origin_time in the future leads to invalid Dna action commit
    let dna_def_original = workspace.dna_def();
    dna_def.modifiers.origin_time = Timestamp::MAX;
    action.hash = DnaHash::with_data_sync(&dna_def);
    workspace.dna_def = Arc::new(dna_def);

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::InvalidRootOriginTime,
                ..
            })
        ))
    );

    action.hash = DnaHash::with_data_sync(&*dna_def_original);
    action.author = fake_agent_pubkey_1();
    workspace.dna_def = dna_def_original;

    check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()).unwrap();

    fake_genesis_for_agent(
        db.clone(),
        tmp_dht.to_db(),
        action.author.clone(),
        keystore,
    )
    .await
    .unwrap();

    tmp_dht
        .to_db()
        .write_async(move |txn| -> DatabaseResult<usize> {
            Ok(txn.execute("UPDATE DhtOp SET when_integrated = 0", [])?)
        })
        .await
        .unwrap();

    cache
        .set_all_activity_to_integrated(vec![(Arc::new(action.author.clone()), 0..=2)])
        .await
        .unwrap();
}

/// Timestamps must increase monotonically
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_timestamp() {
    let mut g = random_generator();

    let before = Timestamp::from(chrono::Utc::now() - chrono::Duration::try_weeks(1).unwrap());
    let after = Timestamp::from(chrono::Utc::now() + chrono::Duration::try_weeks(1).unwrap());

    let keystore = test_keystore();
    let mut action: Action = CreateLink::arbitrary(&mut g).unwrap().into();
    *action.timestamp_mut() = Timestamp::now();
    let (record, mut deps) = record_with_deps(&keystore, action).await;
    *deps[0].as_action_mut().timestamp_mut() = before;

    sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
        .await
        .unwrap();

    *deps[0].as_action_mut().timestamp_mut() = after;
    let r = sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
        .await
        .unwrap_err()
        .into_outcome();

    assert_matches!(
        r,
        Some(ValidationOutcome::PrevActionError(PrevActionError {
            source: PrevActionErrorKind::Timestamp(_, _),
            ..
        }))
    );
}

/// Sequence numbers must increment by 1 for each new action
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_seq() {
    let mut g = random_generator();

    let keystore = test_keystore();
    let mut action: Action = CreateLink::arbitrary(&mut g).unwrap().into();
    *action.action_seq_mut().unwrap() = 2;
    let (mut record, mut deps) = record_with_deps(&keystore, action).await;

    // *record.as_action_mut().action_seq_mut().unwrap() = 2;
    *deps[0].as_action_mut().action_seq_mut().unwrap() = 1;

    assert!(
        sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
            .await
            .is_ok()
    );

    *deps[0].as_action_mut().action_seq_mut().unwrap() = 2;
    assert_matches!(
        sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(PrevActionError {
            source: PrevActionErrorKind::InvalidSeq(2, 2),
            ..
        }))
    );

    *deps[0].as_action_mut().action_seq_mut().unwrap() = 3;
    assert_matches!(
        sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(PrevActionError {
            source: PrevActionErrorKind::InvalidSeq(2, 3),
            ..
        }))
    );

    *record.as_action_mut().action_seq_mut().unwrap() = 0;
    let record = rebuild_record(record, &keystore).await;
    *deps[0].as_action_mut().action_seq_mut().unwrap() = 0;
    assert_matches!(
        sys_validate_record(&record, Arc::new(MockCascade::with_records(deps.clone())))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(PrevActionError {
            source: PrevActionErrorKind::InvalidRoot,
            ..
        }))
    );
}

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
    let mut g = random_generator();

    let mut ec = Create::arbitrary(&mut g).unwrap();
    let entry = Entry::arbitrary(&mut g).unwrap();
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
        check_new_entry_action(&CreateLink::arbitrary(&mut g).unwrap().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::NotNewEntry(_)
        ))
    );
}

/// The size of an entry does not exceed the max
#[tokio::test(flavor = "multi_thread")]
async fn check_entry_size_test() {
    let mut g = random_generator();

    let keystore = test_keystore();

    let action = contrafact::brute("app entry create", |a: &Create| {
        matches!(a.entry_type, EntryType::App(_))
    })
    .build(&mut g);

    let (mut record, cascade) = record_with_cascade(&keystore, action.into()).await;

    let tiny_entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        (0..5).map(|_| 0u8).collect::<Vec<_>>(),
    ))));
    *record.as_action_mut().entry_data_mut().unwrap().1 =
        EntryType::App(AppEntryDef::arbitrary(&mut g).unwrap());
    *record.as_entry_mut() = RecordEntry::Present(tiny_entry);
    let mut record = rebuild_record(record, &keystore).await;
    sys_validate_record(&record, cascade.clone()).await.unwrap();

    let huge_entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        (0..5_000_000).map(|_| 0u8).collect::<Vec<_>>(),
    ))));
    *record.as_entry_mut() = RecordEntry::Present(huge_entry);
    let record = rebuild_record(record, &keystore).await;

    assert_eq!(
        sys_validate_record(&record, cascade)
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::EntryTooLarge(5_000_000))
    );
}

/// Check that updates can't switch the entry type
#[tokio::test(flavor = "multi_thread")]
async fn check_update_reference_test() {
    let mut g = random_generator();

    let keystore = test_keystore();

    let action = contrafact::brute("non agent entry type", move |a: &Action| {
        matches!(
            a,
            Action::Update(Update {
                entry_type: EntryType::App(_),
                ..
            })
        ) && !matches!(a, Action::AgentValidationPkg(..))
            && a.entry_type()
                .map(|et| *et != EntryType::AgentPubKey)
                .unwrap_or(false)
    })
    .build(&mut g);
    let (mut record, cascade) = record_with_cascade(&keystore, action).await;

    let entry_type = record.action().entry_type().unwrap().clone();
    let et2 = entry_type.clone();
    let new_entry_type = contrafact::brute("different entry type", move |et: &EntryType| {
        *et != et2 && *et != EntryType::AgentPubKey
    })
    .build(&mut g);

    let net = new_entry_type.clone();
    let entry = contrafact::brute("matching entry, not countersigning", move |e: &Entry| {
        !matches!(e, Entry::CounterSign(_, _)) && entry_type_matches(&net, e)
    })
    .build(&mut g);

    *record.as_action_mut().entry_data_mut().unwrap().1 = new_entry_type.clone();
    *record.as_entry_mut() = RecordEntry::Present(entry);
    let record = rebuild_record(record, &keystore).await;

    assert_eq!(
        sys_validate_record(&record, cascade)
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::UpdateTypeMismatch(
            entry_type,
            new_entry_type
        ))
    );
}

/// The link tag size is bounded
#[tokio::test(flavor = "multi_thread")]
async fn check_link_tag_size_test() {
    let mut g = random_generator();

    let keystore = test_keystore();

    let bytes = (0..super::MAX_TAG_SIZE + 1)
        .map(|_| 0u8)
        .collect::<Vec<_>>();
    let huge = LinkTag(bytes);

    let mut action = CreateLink::arbitrary(&mut g).unwrap();
    action.tag = huge;
    let (record, cascade) = record_with_cascade(&keystore, action.into()).await;

    assert_eq!(
        sys_validate_record(&record, cascade)
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::TagTooLarge(super::MAX_TAG_SIZE + 1))
    );
}

/// Check that StoreEntry does not have a private entry type
#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_filters_private_entry() {
    let mut g = random_generator();

    let dna = DnaHash::arbitrary(&mut g).unwrap();
    let spaces = TestSpaces::new([dna.clone()]);
    let space = Arc::new(spaces.test_spaces[&dna].space.clone());
    let vault = space.dht_db.clone();
    let keystore = test_keystore();
    let (tx, _rx) = TriggerSender::new();

    let private_entry = Entry::arbitrary(&mut g).unwrap();
    let mut create = Create::arbitrary(&mut g).unwrap();
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let app_entry_def = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    create.entry_type = EntryType::App(app_entry_def);
    create.entry_hash = EntryHash::with_data_sync(&private_entry);
    create.author = author.clone();
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

#[test]
/// Test that a given sequence of actions constitutes a valid chain wrt
/// its backlinks
fn valid_chain_test() {
    isotest::isotest!(TestChainItem, TestChainHash => |iso_a, iso_h| {
        // Create a valid chain.
        let actions = vec![
            iso_a.create(TestChainItem::new(0)),
            iso_a.create(TestChainItem::new(1)),
            iso_a.create(TestChainItem::new(2)),
        ];
        // Valid chain passes.
        validate_chain(actions.iter(), &None).expect("Valid chain");

        // Create a forked chain.
        let mut fork = actions.clone();
        fork.push(iso_a.create(TestChainItem {
            seq: 1,
            hash: 111.into(),
            prev: Some(0.into()),
        }));
        let err = validate_chain(fork.iter(), &None).expect_err("Forked chain");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::HashMismatch(_),
                ..
            }))
        );

        // Test a chain with the wrong seq.
        let mut wrong_seq = actions.clone();
        iso_a.mutate(&mut wrong_seq[2], |s| s.seq = 3);
        let err = validate_chain(wrong_seq.iter(), &None).expect_err("Wrong seq");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::InvalidSeq(_, _),
                ..
            }

            ))
        );

        // Test a wrong root gets rejected.
        let mut wrong_root = actions.clone();
        iso_a.mutate(&mut wrong_root[0], |a| {
            a.prev = Some(0.into());
        });

        let err = validate_chain(wrong_root.iter(), &None).expect_err("Wrong root");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::InvalidRoot,
                ..
            }

            ))
        );

        // Test without dna at root gets rejected.
        let mut dna_not_at_root = actions.clone();
        dna_not_at_root.push(actions[0].clone());
        let err = validate_chain(dna_not_at_root.iter(), &None).expect_err("Dna not at root");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::MissingPrev,
                ..
            }

            ))
        );

        // Test if there is a existing head that a dna in the new chain is rejected.
        let hash = iso_h.create(TestChainHash(123));
        let err = validate_chain(actions.iter(), &Some((hash, 0))).expect_err("Dna not at root");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::MissingPrev,
                ..
            }

            ))
        );

        // Check a sequence that is broken gets rejected.
        let mut wrong_seq = actions[1..].to_vec();
        iso_a.mutate(&mut wrong_seq[0], |s| s.seq = 3);
        iso_a.mutate(&mut wrong_seq[1], |s| s.seq = 4);

        let err = validate_chain(
            wrong_seq.iter(),
            &Some((wrong_seq[0].prev_hash().cloned().unwrap(), 0)),
        )
        .expect_err("Wrong seq");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::InvalidSeq(_, _),
                ..
            }

            ))
        );

        // Check the correct sequence gets accepted with a root.
        let correct_seq = actions[1..].to_vec();
        validate_chain(
            correct_seq.iter(),
            &Some((correct_seq[0].prev_hash().cloned().unwrap(), 0)),
        )
        .expect("Correct seq");

        let hash = iso_h.create(TestChainHash(234));
        let err = validate_chain(correct_seq.iter(), &Some((hash, 0))).expect_err("Hash is wrong");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(PrevActionError {
                source: PrevActionErrorKind::HashMismatch(_),
                ..
            }

            ))
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "dpki")]
async fn test_dpki_agent_update() {
    use crate::core::workflow::inline_validation;
    use crate::sweettest::SweetAgents;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use holochain_p2p::actor::HolochainP2pRefToDna;

    let dna = SweetDnaFile::unique_empty().await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agents = SweetAgents::get(conductor.keystore(), 4).await;
    conductor
        .setup_app_for_agent("app", agents[0].clone(), vec![&dna])
        .await
        .unwrap();

    let dna_hash = dna.dna_hash().clone();
    let space = conductor
        .get_spaces()
        .get_or_create_space(&dna_hash)
        .unwrap();

    let workspace = space
        .source_chain_workspace(
            conductor.keystore(),
            agents[0].clone(),
            Arc::new(dna.dna_def().clone()),
        )
        .await
        .unwrap();
    let chain = workspace.source_chain().clone();

    assert_eq!(chain.len().unwrap(), 3);

    let network = conductor.holochain_p2p().to_dna(dna_hash.clone(), None);
    let ribosome = conductor.get_ribosome(&dna_hash).unwrap();

    let sec = std::time::Duration::from_secs(1);

    let head = chain.chain_head().unwrap().unwrap();

    let a1 = Action::AgentValidationPkg(AgentValidationPkg {
        author: agents[0].clone(),
        timestamp: (head.timestamp + sec).unwrap(),
        action_seq: head.seq + 1,
        prev_action: head.action.clone(),
        membrane_proof: None,
    });

    let a2 = Action::Update(Update {
        author: agents[0].clone(),
        timestamp: (a1.timestamp() + sec).unwrap(),
        action_seq: a1.action_seq() + 1,
        prev_action: a1.to_hash(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agents[1].clone().into(),
        original_action_address: head.action,
        original_entry_address: agents[0].clone().into(),
        weight: EntryRateWeight::default(),
    });

    let a3 = Action::AgentValidationPkg(AgentValidationPkg {
        author: agents[1].clone(),
        timestamp: (a2.timestamp() + sec).unwrap(),
        action_seq: a2.action_seq() + 1,
        prev_action: a2.to_hash(),
        membrane_proof: None,
    });

    let a4 = Action::Update(Update {
        author: agents[1].clone(),
        timestamp: (a3.timestamp() + sec).unwrap(),
        action_seq: a3.action_seq() + 1,
        prev_action: a3.to_hash(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agents[2].clone().into(),
        original_action_address: ActionHash::with_data_sync(&a2),
        original_entry_address: agents[1].clone().into(),
        weight: EntryRateWeight::default(),
    });

    let a5 = Action::Update(Update {
        author: agents[2].clone(),
        timestamp: (a4.timestamp() + sec).unwrap(),
        action_seq: a4.action_seq() + 1,
        prev_action: a4.to_hash(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agents[3].clone().into(),
        original_action_address: ActionHash::with_data_sync(&a4),
        original_entry_address: agents[2].clone().into(),
        weight: EntryRateWeight::default(),
    });

    chain
        .put_with_action(a1, None, ChainTopOrdering::Strict)
        .await
        .unwrap();
    chain
        .put_with_action(
            a2,
            Some(Entry::Agent(agents[1].clone())),
            ChainTopOrdering::Strict,
        )
        .await
        .unwrap();
    chain
        .put_with_action(a3, None, ChainTopOrdering::Strict)
        .await
        .unwrap();
    chain
        .put_with_action(
            a4,
            Some(Entry::Agent(agents[2].clone())),
            ChainTopOrdering::Strict,
        )
        .await
        .unwrap();

    inline_validation(
        workspace.clone(),
        network.clone(),
        conductor.raw_handle(),
        ribosome.clone(),
    )
    .await
    .unwrap();

    chain
        .put_with_action(
            a5,
            Some(Entry::Agent(agents[3].clone())),
            ChainTopOrdering::Strict,
        )
        .await
        .unwrap();
    // this should be invalid
    inline_validation(workspace, network, conductor.raw_handle(), ribosome.clone())
        .await
        .unwrap_err();
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
    let mut g = random_generator();

    let mut chain = valid_arbitrary_chain(&mut g, keystore.clone(), author, n).await;

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
