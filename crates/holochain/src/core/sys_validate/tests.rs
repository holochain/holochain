use super::*;
use crate::conductor::handle::MockConductorHandleT;
use crate::conductor::space::TestSpaces;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::test_utils::fake_genesis;
use ::fixt::prelude::*;
use error::SysValidationError;

use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::test_authored_db;
use holochain_state::prelude::test_cache_db;
use holochain_state::prelude::test_dht_db;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::test_utils::chain::TestChainItem;
use holochain_wasm_test_utils::*;
use holochain_zome_types::Action;
use matches::assert_matches;
use observability;
use std::convert::TryFrom;

#[tokio::test(flavor = "multi_thread")]
async fn verify_action_signature_test() {
    let keystore = holochain_state::test_utils::test_keystore();
    let author = fake_agent_pubkey_1();
    let mut action = fixt!(CreateLink);
    action.author = author.clone();
    let action = Action::CreateLink(action);
    let real_signature = author.sign(&keystore, &action).await.unwrap();
    let wrong_signature = Signature([1_u8; 64]);

    assert_matches!(
        verify_action_signature(&wrong_signature, &action).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::Counterfeit(_, _)
        ))
    );

    assert_matches!(
        verify_action_signature(&real_signature, &action).await,
        Ok(())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_action() {
    let mut action = fixt!(CreateLink);
    action.prev_action = fixt!(ActionHash);
    action.action_seq = 1;
    assert_matches!(check_prev_action(&action.clone().into()), Ok(()));
    action.action_seq = 0;
    assert_matches!(
        check_prev_action(&action.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidRoot)
        ))
    );
    // Dna is always ok because of the type system
    let action = fixt!(Dna);
    assert_matches!(check_prev_action(&action.into()), Ok(()));
}

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
    dna_def.origin_time = Timestamp::MIN;

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

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace).await,
        Ok(())
    );
    let mut action = fixt!(Dna);

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace).await,
        Ok(())
    );

    // - Test that an origin_time in the future leads to invalid Dna action commit
    let dna_def_original = workspace.dna_def();
    dna_def.origin_time = Timestamp::MAX;
    workspace.dna_def = Arc::new(dna_def);
    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidRootOriginTime)
        ))
    );
    workspace.dna_def = dna_def_original;

    fake_genesis(db.clone().into(), tmp_dht.to_db(), keystore)
        .await
        .unwrap();
    tmp_dht
        .to_db()
        .conn()
        .unwrap()
        .execute("UPDATE DhtOp SET when_integrated = 0", [])
        .unwrap();

    action.author = fake_agent_pubkey_1();

    cache
        .set_all_activity_to_integrated(vec![(Arc::new(action.author.clone()), 0..=2)])
        .await
        .unwrap();

    assert_matches!(
        check_valid_if_dna(&action.clone().into(), &workspace).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidRoot)
        ))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_timestamp() {
    let mut action = fixt!(CreateLink);
    let mut prev_action = fixt!(CreateLink);
    action.timestamp = Timestamp::now().into();
    let before = chrono::Utc::now() - chrono::Duration::weeks(1);
    let after = chrono::Utc::now() + chrono::Duration::weeks(1);

    prev_action.timestamp = Timestamp::from(before).into();
    let r = check_prev_timestamp(&action.clone().into(), &prev_action.clone().into());
    assert_matches!(r, Ok(()));

    prev_action.timestamp = Timestamp::from(after).into();
    let r = check_prev_timestamp(&action.clone().into(), &prev_action.clone().into());
    assert_matches!(
        r,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::Timestamp)
        ))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_seq() {
    let mut action = fixt!(CreateLink);
    let mut prev_action = fixt!(CreateLink);

    action.action_seq = 2;
    prev_action.action_seq = 1;
    assert_matches!(
        check_prev_seq(&action.clone().into(), &prev_action.clone().into()),
        Ok(())
    );

    prev_action.action_seq = 2;
    assert_matches!(
        check_prev_seq(&action.clone().into(), &prev_action.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidSeq(_, _)),
        ),)
    );

    prev_action.action_seq = 3;
    assert_matches!(
        check_prev_seq(&action.clone().into(), &prev_action.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidSeq(_, _)),
        ),)
    );

    action.action_seq = 0;
    prev_action.action_seq = 0;
    assert_matches!(
        check_prev_seq(&action.clone().into(), &prev_action.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevActionError(PrevActionError::InvalidSeq(_, _)),
        ),)
    );
}

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
                ValidationOutcome::EntryType
            ))
        );
    }
}

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

#[tokio::test(flavor = "multi_thread")]
async fn check_entry_size_test() {
    // let tiny = Entry::App(SerializedBytes::from(UnsafeBytes::from(vec![0; 1])));
    // let bytes = (0..16_000_000).map(|_| 0u8).into_iter().collect::<Vec<_>>();
    // let huge = Entry::App(SerializedBytes::from(UnsafeBytes::from(bytes)));
    // assert_matches!(check_entry_size(&tiny), Ok(()));

    // assert_matches!(
    //     check_entry_size(&huge),
    //     Err(SysValidationError::ValidationOutcome(ValidationOutcome::EntryTooLarge(_, _)))
    // );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_update_reference_test() {
    let mut ec = fixt!(Create);
    let mut eu = fixt!(Update);
    let et_cap = EntryType::CapClaim;
    let mut aet_fixt = AppEntryTypeFixturator::new(Predictable).map(EntryType::App);
    let et_app_1 = aet_fixt.next().unwrap();
    let et_app_2 = aet_fixt.next().unwrap();

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

#[tokio::test(flavor = "multi_thread")]
async fn check_app_entry_type_test() {
    observability::test_run().ok();
    let TestWasmPair::<DnaWasm> {
        integrity,
        coordinator,
    } = TestWasm::EntryDefs.into();
    // Setup test data
    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_entry_type_test".to_string(),
            network_seed: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            origin_time: Timestamp::HOLOCHAIN_EPOCH,
            integrity_zomes: vec![TestZomes::from(TestWasm::EntryDefs).integrity.into_inner()],
            coordinator_zomes: vec![TestZomes::from(TestWasm::EntryDefs)
                .coordinator
                .into_inner()],
        },
        [integrity, coordinator],
    )
    .await
    .unwrap();
    let dna_hash = dna_file.dna_hash().to_owned().clone();
    let ribosome = RealRibosome::new(dna_file).unwrap();
    let mut entry_def = fixt!(EntryDef);
    entry_def.visibility = EntryVisibility::Public;

    // Setup mock conductor
    let mut conductor_handle = MockConductorHandleT::new();
    // # No dna or entry def
    let dh = dna_hash.clone();
    conductor_handle
        .expect_get_ribosome()
        .returning(move |_| Err(DnaError::DnaMissing(dh.to_owned()).into()));

    // ## Dna is missing
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeId out of range
    conductor_handle.checkpoint();
    let r = ribosome.clone();
    conductor_handle
        .expect_get_ribosome()
        .returning(move |_| Ok(r.clone()));
    conductor_handle.expect_get_entry_def().return_const(None);

    // ## EntryId is out of range
    let aet = AppEntryType::new(10.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryDefId(_)
        ))
    );

    let aet = AppEntryType::new(0.into(), 100.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::ZomeId(_)
        ))
    );

    // ## EntryId is in range for dna
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Ok(_)
    );
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Private);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryVisibility(_)
        ))
    );

    // # Add an entry def to the buffer
    conductor_handle
        .expect_get_entry_def()
        .return_const(Some(entry_def));

    // ## Can get the entry from the entry def
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Ok(_)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_entry_not_private_test() {
    let mut ed = fixt!(EntryDef);
    ed.visibility = EntryVisibility::Public;
    assert_matches!(check_not_private(&ed), Ok(()));

    ed.visibility = EntryVisibility::Private;
    assert_matches!(
        check_not_private(&ed),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrivateEntry
        ))
    );
}

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
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Private);
    create.entry_type = EntryType::App(aet);
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
/// Test the chain validation works.
fn valid_chain_test() {
    // Create a valid chain.
    let actions = vec![
        TestChainItem::new(0),
        TestChainItem::new(1),
        TestChainItem::new(2),
    ];
    // Valid chain passes.
    validate_chain(actions.iter(), &None).expect("Valid chain");

    // Create a forked chain.
    let mut fork = actions.clone();
    fork.push(TestChainItem {
        seq: 1,
        hash: 111.into(),
        prev: Some(0.into()),
    });
    let err = validate_chain(fork.iter(), &None).expect_err("Forked chain");
    assert_matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
            PrevActionError::HashMismatch(_)
        ))
    );

    // Test a chain with the wrong seq.
    let mut wrong_seq = actions.clone();
    wrong_seq[2].seq = 3;
    let err = validate_chain(wrong_seq.iter(), &None).expect_err("Wrong seq");
    assert_matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
            PrevActionError::InvalidSeq(_, _)
        ))
    );

    // Test a wrong root gets rejected.
    let mut wrong_root = actions.clone();
    wrong_root[0].prev = Some(0.into());
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
    let err = validate_chain(actions.iter(), &Some((123.into(), 0))).expect_err("Dna not at root");
    assert_matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
            PrevActionError::MissingPrev
        ))
    );

    // Check a sequence that is broken gets rejected.
    let mut wrong_seq = actions[1..].to_vec();
    wrong_seq[0].seq = 3;
    wrong_seq[1].seq = 4;
    let err = validate_chain(
        wrong_seq.iter(),
        &Some((wrong_seq[0].prev.clone().unwrap(), 0)),
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
        &Some((correct_seq[0].prev.clone().unwrap(), 0)),
    )
    .expect("Correct seq");

    let err =
        validate_chain(correct_seq.iter(), &Some((234.into(), 0))).expect_err("Hash is wrong");
    assert_matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevActionError(
            PrevActionError::HashMismatch(_)
        ))
    );
}
