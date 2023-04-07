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
use crate::core::workflow::inline_validation;
use crate::core::workflow::sys_validation_workflow::sys_validate_record;
use crate::sweettest::SweetAgents;
use crate::sweettest::SweetConductor;
use crate::sweettest::SweetDnaFile;
use crate::test_utils::fake_genesis_for_agent;
use ::fixt::prelude::*;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use error::SysValidationError;

use holochain_cascade::MockCascade;
use holochain_keystore::AgentPubKeyExt;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::test_authored_db;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_state::test_utils::test_db_dir;
use holochain_trace;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::test_utils::chain::{TestChainHash, TestChainItem};
use holochain_wasm_test_utils::*;
use holochain_zome_types::Action;
use matches::assert_matches;
use std::convert::TryFrom;
use std::time::Duration;

async fn sign_record(keystore: &MetaLairClient, action: Action, entry: Option<Entry>) -> Record {
    Record::new(
        SignedActionHashed::sign(keystore, ActionHashed::from_content_sync(action))
            .await
            .unwrap(),
        entry,
    )
}

pub async fn rebuild_record(record: Record, keystore: &MetaLairClient) -> Record {
    let (action, entry) = record.into_inner();
    sign_record(
        keystore,
        action.into_inner().0.into_content(),
        entry.into_option(),
    )
    .await
}

fn matching_record(u: &mut Unstructured, f: impl Fn(&Record) -> bool) -> Record {
    // get anything but a Dna
    let mut dep = Record::arbitrary(u).unwrap();
    while !f(&dep) {
        dep = Record::arbitrary(u).unwrap();
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
    let mut u = unstructured_noise();

    if fixup {
        if action.action_seq() == 1 {
            // In case this is an Agent entry, allow the previous to be something other than Dna
            *action.action_seq_mut().unwrap() = 2;
        }

        *action.author_mut() = fake_agent_pubkey_1();
    }

    let (entry, deps) = match &mut action {
        Action::Dna(_) => (None, vec![]),
        action if fixup && action.action_seq() == 0 => {
            *action = Action::Dna(fixt!(Dna));
            *action.author_mut() = fake_agent_pubkey_1();
            (None, vec![])
        }
        action => {
            let mut deps = vec![];
            let prev_seq = action.action_seq() - 1;
            let mut prev = if prev_seq == 0 {
                matching_record(&mut u, is_dna_record)
            } else {
                let mut prev = match action {
                    Action::Create(Create {
                        entry_type: EntryType::AgentPubKey,
                        ..
                    }) => matching_record(&mut u, is_pkg_record),
                    _ => matching_record(&mut u, |r| !is_dna_record(r) && !is_pkg_record(r)),
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

            let entry = action.entry_data_mut().map(|(entry_hash, _)| {
                let entry = fixt!(Entry);
                *entry_hash = EntryHash::with_data_sync(&entry);
                entry
            });
            match action {
                Action::Update(update) => {
                    let mut dep = matching_record(&mut u, is_entry_record);
                    update.original_action_address = dep.action_address().clone();
                    update.original_entry_address =
                        dep.entry().as_option().unwrap().to_hash().clone();
                    *dep.as_action_mut().entry_data_mut().unwrap().1 = update.entry_type.clone();
                    deps.push(dep);
                }
                Action::Delete(delete) => {
                    let dep = matching_record(&mut u, is_entry_record);
                    delete.deletes_address = dep.action_address().clone();
                    delete.deletes_entry_address =
                        dep.entry().as_option().unwrap().to_hash().clone();

                    deps.push(dep);
                }
                Action::CreateLink(link) => {
                    let base = fixt!(Record);
                    let target = fixt!(Record);
                    link.base_address = base.action_address().clone().into();
                    link.target_address = target.action_address().clone().into();
                    deps.push(base);
                    deps.push(target);
                }
                Action::DeleteLink(delete) => {
                    let base = fixt!(Record);
                    let create =
                        matching_record(&mut u, |r| matches!(r.action(), Action::CreateLink(_)));
                    delete.base_address = base.action_address().clone().into();
                    delete.link_add_address = create.action_address().clone().into();
                    deps.push(base);
                    deps.push(create);
                }
                Action::Create(_)
                | Action::AgentValidationPkg(_)
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

async fn record_with_cascade(keystore: &MetaLairClient, action: Action) -> (Record, MockCascade) {
    let (record, deps) = record_with_deps(keystore, action).await;
    (record, MockCascade::with_records(deps))
}

async fn validate_action(keystore: &MetaLairClient, action: Action) -> SysValidationOutcome<()> {
    let (record, deps) = record_with_deps(keystore, action).await;
    let cascade = MockCascade::with_records(deps.clone());
    dbg!(&deps, &record);
    sys_validate_record(&record, &cascade).await
}

async fn assert_valid_action(keystore: &MetaLairClient, action: Action) {
    let (record, deps) = record_with_deps(keystore, action).await;
    let cascade = MockCascade::with_records(deps.clone());
    let result = sys_validate_record(&record, &cascade).await;
    if result.is_err() {
        dbg!(&deps, &record);
        result.unwrap()
    }
}

async fn assert_invalid_action(keystore: &MetaLairClient, action: Action) {
    let (record, deps) = record_with_deps(keystore, action).await;
    let cascade = MockCascade::with_records(deps.clone());
    let result = sys_validate_record(&record, &cascade).await;
    if result.is_ok() {
        dbg!(&deps, &record);
        result.unwrap_err();
    }
}

/// Mismatched signatures are rejected
#[tokio::test(flavor = "multi_thread")]
async fn test_record_with_cascade() {
    let keystore = holochain_state::test_utils::test_keystore();
    for _ in 0..100 {
        assert_valid_action(&keystore, fixt!(Action)).await;
    }
}

/// Mismatched signatures are rejected
#[tokio::test(flavor = "multi_thread")]
async fn verify_action_signature_test() {
    let keystore = holochain_state::test_utils::test_keystore();
    let action = fixt!(CreateLink);
    let (record_valid, cascade) = record_with_cascade(&keystore, Action::CreateLink(action)).await;

    let wrong_signature = Signature([1_u8; 64]);
    let action_invalid = SignedActionHashed::new(record_valid.action().clone(), wrong_signature);
    let record_invalid = Record::new(action_invalid, None);

    sys_validate_record(&record_valid, &cascade).await.unwrap();
    sys_validate_record(&record_invalid, &cascade)
        .await
        .unwrap_err();
}

/// Any action other than DNA cannot be at seq 0
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_action() {
    let mut u = unstructured_noise();
    let keystore = holochain_state::test_utils::test_keystore();
    let mut action = Action::Delete(fixt!(Delete));
    *action.author_mut() = keystore.new_sign_keypair_random().await.unwrap();

    *action.action_seq_mut().unwrap() = 7;

    assert_valid_action(&keystore, action.clone()).await;

    *action.action_seq_mut().unwrap() = 0;

    // This check is manual because `validate_action` will modify any action
    // coming in with a 0 action_seq since it knows that can't be valid.
    {
        let actual = sys_validate_record(
            &sign_record(&keystore, action, None).await,
            &MockCascade::new(),
        )
        .await
        .unwrap_err()
        .into_outcome();

        let expected = Some(ValidationOutcome::PrevActionError(
            PrevActionError::InvalidRoot,
        ));
        assert_eq!(actual, expected);
    }

    // Dna is always ok because of the type system
    let action = Action::Dna(fixt!(Dna));
    assert_valid_action(&keystore, action.clone()).await;
}

/// The DNA action can only be validated if the chain is empty,
/// and its timestamp must not be less than the origin time
/// (this "if chain not empty" thing is a bit weird, TODO refactor to not look in the db)
#[tokio::test(flavor = "multi_thread")]
async fn check_valid_if_dna_test() {
    let tmp = test_authored_db();
    let tmp_dht = test_dht_db();
    let tmp_cache = test_cache_db();
    let keystore = test_keystore();
    let db = tmp.to_db();
    // Test data
    let _activity_return = vec![fixt!(ActionHash)];

    let mut dna_def = fixt!(DnaDef);
    dna_def.modifiers.origin_time = Timestamp::MIN;

    // Empty store not dna
    let action = fixt!(CreateLink);
    let cache: DhtDbQueryCache = tmp_dht.to_db().into();
    let mut workspace = SysValidationWorkspace::new(
        db.clone().into(),
        tmp_dht.to_db().into(),
        cache.clone(),
        tmp_cache.to_db(),
        Arc::new(dna_def.clone()),
    );

    // Initializing the cache actually matters. TODO: why?
    cache.get_state().await;

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()),
        Ok(())
    );
    let mut action = fixt!(Dna);
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
            ValidationOutcome::PrevActionError(PrevActionError::InvalidRootOriginTime)
        ))
    );

    action.hash = DnaHash::with_data_sync(&*dna_def_original);
    action.author = fake_agent_pubkey_1();
    workspace.dna_def = dna_def_original;

    check_valid_if_dna(&action.clone().into(), &workspace.dna_def_hashed()).unwrap();

    fake_genesis_for_agent(
        db.clone().into(),
        tmp_dht.to_db(),
        action.author.clone(),
        keystore,
    )
    .await
    .unwrap();

    tmp_dht
        .to_db()
        .conn()
        .unwrap()
        .execute("UPDATE DhtOp SET when_integrated = 0", [])
        .unwrap();

    cache
        .set_all_activity_to_integrated(vec![(Arc::new(action.author.clone()), 0..=2)])
        .await
        .unwrap();
}

/// Timestamps must increase monotonically
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_timestamp() {
    let before = Timestamp::from(chrono::Utc::now() - chrono::Duration::weeks(1));
    let after = Timestamp::from(chrono::Utc::now() + chrono::Duration::weeks(1));

    let keystore = test_keystore();
    let (record, mut deps) = record_with_deps(&keystore, fixt!(CreateLink).into()).await;
    *deps[0].as_action_mut().timestamp_mut() = before;

    assert!(
        sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
            .await
            .is_ok()
    );

    *deps[0].as_action_mut().timestamp_mut() = after;
    let r = sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
        .await
        .unwrap_err()
        .into_outcome();

    assert_matches!(
        r,
        Some(ValidationOutcome::PrevActionError(
            PrevActionError::Timestamp(_, _)
        ))
    );
}

/// Sequence numbers must increment by 1 for each new action
#[tokio::test(flavor = "multi_thread")]
async fn check_previous_seq() {
    let keystore = test_keystore();
    let mut action: Action = fixt!(CreateLink).into();
    *action.action_seq_mut().unwrap() = 2;
    let (mut record, mut deps) = record_with_deps(&keystore, action).await;

    dbg!(&deps, &record);
    // *record.as_action_mut().action_seq_mut().unwrap() = 2;
    *deps[0].as_action_mut().action_seq_mut().unwrap() = 1;

    assert!(
        sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
            .await
            .is_ok()
    );

    *deps[0].as_action_mut().action_seq_mut().unwrap() = 2;
    assert_eq!(
        sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(
            PrevActionError::InvalidSeq(2, 2)
        )),
    );

    *deps[0].as_action_mut().action_seq_mut().unwrap() = 3;
    assert_eq!(
        sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(
            PrevActionError::InvalidSeq(2, 3)
        )),
    );

    *record.as_action_mut().action_seq_mut().unwrap() = 0;
    let record = rebuild_record(record, &keystore).await;
    *deps[0].as_action_mut().action_seq_mut().unwrap() = 0;
    assert_eq!(
        sys_validate_record(&record, &MockCascade::with_records(deps.clone()))
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::PrevActionError(
            PrevActionError::InvalidRoot
        )),
    );
}

/// Entry type in the action matches the entry variant
#[tokio::test(flavor = "multi_thread")]
async fn check_entry_type_test() {
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
#[tokio::test(flavor = "multi_thread")]
async fn check_entry_hash_test() {
    let mut ec = fixt!(Create);
    let entry = fixt!(Entry);
    let hash = EntryHash::with_data_sync(&entry);
    let action: Action = ec.clone().into();

    // First check it should have an entry
    assert_matches!(check_new_entry_action(&action), Ok(()));
    // Safe to unwrap if new entry
    let eh = action.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(
        check_entry_hash(&eh, &entry).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryHash
        ))
    );

    ec.entry_hash = hash;
    let action: Action = ec.clone().into();

    let eh = action.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(check_entry_hash(&eh, &entry).await, Ok(()));
    assert_matches!(
        check_new_entry_action(&fixt!(CreateLink).into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::NotNewEntry(_)
        ))
    );
}

/// The size of an entry does not exceed the max
#[tokio::test(flavor = "multi_thread")]
async fn check_entry_size_test() {
    let keystore = test_keystore();

    let (mut record, cascade) = record_with_cascade(&keystore, fixt!(Create).into()).await;

    assert!(sys_validate_record(&record, &cascade).await.is_ok());

    let huge_entry = Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(
        (0..5_000_000).map(|_| 0u8).into_iter().collect::<Vec<_>>(),
    ))));
    match record.as_entry_mut() {
        RecordEntry::Present(entry) => *entry = huge_entry,
        _ => {}
    };
    let record = rebuild_record(record, &keystore).await;

    assert_eq!(
        sys_validate_record(&record, &cascade)
            .await
            .unwrap_err()
            .into_outcome(),
        Some(ValidationOutcome::EntryTooLarge(5_000_000))
    );
}

/// Check that updates can't switch the entry type
#[tokio::test(flavor = "multi_thread")]
async fn check_update_reference_test() {
    let mut ec = fixt!(Create);
    let mut eu = fixt!(Update);
    let et_cap = EntryType::CapClaim;
    let mut app_entry_def_fixt = AppEntryDefFixturator::new(Predictable).map(EntryType::App);
    let et_app_1 = app_entry_def_fixt.next().unwrap();
    let et_app_2 = app_entry_def_fixt.next().unwrap();

    // Same entry type
    ec.entry_type = et_app_1.clone();
    eu.entry_type = et_app_1;

    assert_matches!(
        check_update_reference(&eu, &NewEntryActionRef::from(&ec)),
        Ok(())
    );

    // Different app entry type
    ec.entry_type = et_app_2;

    assert_matches!(
        check_update_reference(&eu, &NewEntryActionRef::from(&ec)),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::UpdateTypeMismatch(_, _)
        ))
    );

    // Different entry type
    eu.entry_type = et_cap;

    assert_matches!(
        check_update_reference(&eu, &NewEntryActionRef::from(&ec)),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::UpdateTypeMismatch(_, _)
        ))
    );
}

/// The link tag size is bounded
#[tokio::test(flavor = "multi_thread")]
async fn check_link_tag_size_test() {
    let tiny = LinkTag(vec![0; 1]);
    let bytes = (0..super::MAX_TAG_SIZE + 1)
        .map(|_| 0u8)
        .into_iter()
        .collect::<Vec<_>>();
    let huge = LinkTag(bytes);
    assert_matches!(check_tag_size(&tiny), Ok(()));

    assert_matches!(
        check_tag_size(&huge),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::TagTooLarge(_, _)
        ))
    );
}

/// Check the AppEntryDef is valid for the zome and the EntryDefId and ZomeIndex are in range.
#[tokio::test(flavor = "multi_thread")]
async fn check_app_entry_def_test() {
    holochain_trace::test_run().ok();
    let TestWasmPair::<DnaWasm> {
        integrity,
        coordinator,
    } = TestWasm::EntryDefs.into();
    // Setup test data
    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_entry_def_test".to_string(),
            modifiers: DnaModifiers {
                network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                origin_time: Timestamp::HOLOCHAIN_EPOCH,
                quantum_time: holochain_p2p::dht::spacetime::STANDARD_QUANTUM_TIME,
            },
            integrity_zomes: vec![TestZomes::from(TestWasm::EntryDefs).integrity.into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::EntryDefs)
                .coordinator
                .into_inner()],
        },
        [integrity, coordinator],
    )
    .await;
    let dna_hash = dna_file.dna_hash().to_owned().clone();
    let mut entry_def = fixt!(EntryDef);
    entry_def.visibility = EntryVisibility::Public;

    let db_dir = test_db_dir();
    let conductor_handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();

    // ## Dna is missing
    let app_entry_def_0 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_0, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeIndex out of range
    conductor_handle.register_dna(dna_file).await.unwrap();

    // ## EntryId is out of range
    let app_entry_def_1 = AppEntryDef::new(10.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_1, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryDefId(_)
        ))
    );

    let app_entry_def_2 = AppEntryDef::new(0.into(), 100.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_2, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::ZomeIndex(_)
        ))
    );

    // ## EntryId is in range for dna
    let app_entry_def_3 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_3, &dna_hash, &conductor_handle).await,
        Ok(_)
    );
    let app_entry_def_4 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    assert_matches!(
        check_app_entry_def(&app_entry_def_4, &dna_hash, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryVisibility(_)
        ))
    );

    // ## Can get the entry from the entry def
    let app_entry_def_5 = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_def(&app_entry_def_5, &dna_hash, &conductor_handle).await,
        Ok(_)
    );
}

/// Check that StoreEntry does not have a private entry type
#[tokio::test(flavor = "multi_thread")]
async fn incoming_ops_filters_private_entry() {
    let dna = fixt!(DnaHash);
    let spaces = TestSpaces::new([dna.clone()]);
    let space = Arc::new(spaces.test_spaces[&dna].space.clone());
    let vault = space.dht_db.clone();
    let keystore = test_keystore();
    let (tx, _rx) = TriggerSender::new();

    let private_entry = fixt!(Entry);
    let mut create = fixt!(Create);
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let app_entry_def = AppEntryDef::new(0.into(), 0.into(), EntryVisibility::Private);
    create.entry_type = EntryType::App(app_entry_def);
    create.entry_hash = EntryHash::with_data_sync(&private_entry);
    create.author = author.clone();
    let action = Action::Create(create);
    let signature = author.sign(&keystore, &action).await.unwrap();

    let shh =
        SignedActionHashed::with_presigned(ActionHashed::from_content_sync(action), signature);
    let el = Record::new(shh, Some(private_entry));

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_entry(el.clone()).await.unwrap();
    let num_ops: usize = fresh_reader_test(vault.clone(), |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))
            .unwrap()
    });
    assert_eq!(num_ops, 0);

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_record(el.clone()).await.unwrap();
    let num_ops: usize = fresh_reader_test(vault.clone(), |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))
            .unwrap()
    });
    assert_eq!(num_ops, 1);
    let num_entries: usize = fresh_reader_test(vault.clone(), |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM Entry", [], |row| row.get(0))
            .unwrap()
    });
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
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::HashMismatch(_)
            ))
        );

        // Test a chain with the wrong seq.
        let mut wrong_seq = actions.clone();
        iso_a.mutate(&mut wrong_seq[2], |s| s.seq = 3);
        let err = validate_chain(wrong_seq.iter(), &None).expect_err("Wrong seq");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::InvalidSeq(_, _)
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
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::InvalidRoot
            ))
        );

        // Test without dna at root gets rejected.
        let mut dna_not_at_root = actions.clone();
        dna_not_at_root.push(actions[0].clone());
        let err = validate_chain(dna_not_at_root.iter(), &None).expect_err("Dna not at root");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::MissingPrev
            ))
        );

        // Test if there is a existing head that a dna in the new chain is rejected.
        let hash = iso_h.create(TestChainHash(123));
        let err = validate_chain(actions.iter(), &Some((hash, 0))).expect_err("Dna not at root");
        assert_matches!(
            err,
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::MissingPrev
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
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::InvalidSeq(_, _)
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
            SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
                PrevActionError::HashMismatch(_)
            ))
        );
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn test_agent_update() {
    let dna = SweetDnaFile::unique_empty().await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let (agent1, agent2, agent3) = SweetAgents::three(conductor.keystore()).await;
    conductor
        .setup_app_for_agent("app", agent1.clone(), vec![&dna])
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
            agent1.clone(),
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
        author: agent2.clone(),
        timestamp: (head.timestamp + sec).unwrap(),
        action_seq: head.seq + 1,
        prev_action: head.action,
        membrane_proof: None,
    });

    let a2 = Action::Create(Create {
        author: agent2.clone(),
        timestamp: (a1.timestamp() + sec).unwrap(),
        action_seq: a1.action_seq() + 1,
        prev_action: a1.to_hash(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agent2.clone().into(),
        weight: EntryRateWeight::default(),
    });

    let a3 = Action::Create(Create {
        author: agent3.clone(),
        timestamp: (a2.timestamp() + sec).unwrap(),
        action_seq: a2.action_seq() + 1,
        prev_action: a2.to_hash(),
        entry_type: EntryType::AgentPubKey,
        entry_hash: agent3.clone().into(),
        weight: EntryRateWeight::default(),
    });

    chain
        .put_with_action(a1, None, ChainTopOrdering::Strict)
        .await
        .unwrap();
    chain
        .put_with_action(a2, Some(Entry::Agent(agent2)), ChainTopOrdering::Strict)
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
        .put_with_action(a3, Some(Entry::Agent(agent3)), ChainTopOrdering::Strict)
        .await
        .unwrap();
    // this should be invalid
    inline_validation(workspace, network, conductor.raw_handle(), ribosome.clone())
        .await
        .unwrap_err();
}

#[tokio::test(flavor = "multi_thread")]
/// Test that the valid_chain contrafact matches our chain validation function,
/// since many other tests will depend on this constraint
async fn valid_chain_fact_test() {
    let keystore = SweetConductor::from_standard_config().await.keystore();
    let author = SweetAgents::one(keystore.clone()).await;

    let mut u = arbitrary::Unstructured::new(&NOISE);
    let fact = contrafact::facts![
        record::facts::action_and_entry_match(),
        contrafact::lens(
            "action is valid",
            |(a, _)| a,
            holochain_zome_types::action::facts::valid_chain(author),
        ),
    ];
    let mut chain: Vec<Record> =
        futures::future::join_all(contrafact::build_seq(&mut u, 100, fact).into_iter().map(
            |(a, entry)| {
                let keystore = keystore.clone();
                async move {
                    Record::new(
                        SignedActionHashed::sign(&keystore, ActionHashed::from_content_sync(a))
                            .await
                            .unwrap(),
                        entry,
                    )
                }
            },
        ))
        .await;

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

    sys_validate_record(&last, &cascade).await.unwrap();
}
