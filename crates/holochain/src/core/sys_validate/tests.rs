use super::*;
use crate::conductor::conductor::RwShare;
use crate::conductor::handle::MockConductorHandleT;
use crate::conductor::space::TestSpaces;
use crate::fixt::DnaFileFixturator;
use crate::test_utils::fake_genesis;
use ::fixt::prelude::*;
use error::SysValidationError;

use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::test_authored_env;
use holochain_state::prelude::test_cache_env;
use holochain_state::prelude::test_dht_env;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::Header;
use matches::assert_matches;
use observability;
use std::convert::TryFrom;

#[tokio::test(flavor = "multi_thread")]
async fn verify_header_signature_test() {
    let keystore = holochain_state::test_utils::test_keystore();
    let author = fake_agent_pubkey_1();
    let mut header = fixt!(CreateLink);
    header.author = author.clone();
    let header = Header::CreateLink(header);
    let real_signature = author.sign(&keystore, &header).await.unwrap();
    let wrong_signature = Signature([1_u8; 64]);

    assert_matches!(
        verify_header_signature(&wrong_signature, &header).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::Counterfeit(_, _)
        ))
    );

    assert_matches!(
        verify_header_signature(&real_signature, &header).await,
        Ok(())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_header() {
    let mut header = fixt!(CreateLink);
    header.prev_header = fixt!(HeaderHash);
    header.header_seq = 1;
    assert_matches!(check_prev_header(&header.clone().into()), Ok(()));
    header.header_seq = 0;
    assert_matches!(
        check_prev_header(&header.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidRoot)
        ))
    );
    // Dna is always ok because of the type system
    let header = fixt!(Dna);
    assert_matches!(check_prev_header(&header.into()), Ok(()));
}

#[tokio::test(flavor = "multi_thread")]
async fn check_valid_if_dna_test() {
    let tmp = test_authored_env();
    let tmp_dht = test_dht_env();
    let tmp_cache = test_cache_env();
    let keystore = test_keystore();
    let env = tmp.env();
    // Test data
    let _activity_return = vec![fixt!(HeaderHash)];

    let mut dna_def = fixt!(DnaDef);
    dna_def.origin_time = Timestamp::MIN;

    // Empty store not dna
    let header = fixt!(CreateLink);
    let mut workspace = SysValidationWorkspace::new(
        env.clone().into(),
        tmp_dht.env().into(),
        tmp_cache.env(),
        Arc::new(dna_def.clone()),
    );

    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &workspace).await,
        Ok(())
    );
    let mut header = fixt!(Dna);

    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &workspace).await,
        Ok(())
    );

    // - Test that an origin_time in the future leads to invalid Dna header commit
    let dna_def_original = workspace.dna_def();
    dna_def.origin_time = Timestamp::MAX;
    workspace.dna_def = Arc::new(dna_def);
    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &workspace).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidRootOriginTime)
        ))
    );
    workspace.dna_def = dna_def_original;

    fake_genesis(env.clone().into(), tmp_dht.env(), keystore)
        .await
        .unwrap();
    tmp_dht
        .env()
        .conn()
        .unwrap()
        .execute("UPDATE DhtOp SET when_integrated = 0", [])
        .unwrap();

    header.author = fake_agent_pubkey_1();

    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &workspace).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidRoot)
        ))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_timestamp() {
    let mut header = fixt!(CreateLink);
    let mut prev_header = fixt!(CreateLink);
    header.timestamp = Timestamp::now().into();
    let before = chrono::Utc::now() - chrono::Duration::weeks(1);
    let after = chrono::Utc::now() + chrono::Duration::weeks(1);

    prev_header.timestamp = Timestamp::from(before).into();
    let r = check_prev_timestamp(&header.clone().into(), &prev_header.clone().into());
    assert_matches!(r, Ok(()));

    prev_header.timestamp = Timestamp::from(after).into();
    let r = check_prev_timestamp(&header.clone().into(), &prev_header.clone().into());
    assert_matches!(
        r,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::Timestamp)
        ))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_seq() {
    let mut header = fixt!(CreateLink);
    let mut prev_header = fixt!(CreateLink);

    header.header_seq = 2;
    prev_header.header_seq = 1;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Ok(())
    );

    prev_header.header_seq = 2;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
        ),)
    );

    prev_header.header_seq = 3;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
        ),)
    );

    header.header_seq = 0;
    prev_header.header_seq = 0;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
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
    let header: Header = ec.clone().into();

    // First check it should have an entry
    assert_matches!(check_new_entry_header(&header), Ok(()));
    // Safe to unwrap if new entry
    let eh = header.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(
        check_entry_hash(&eh, &entry).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryHash
        ))
    );

    ec.entry_hash = hash;
    let header: Header = ec.clone().into();

    let eh = header.entry_data().map(|(h, _)| h).unwrap();
    assert_matches!(check_entry_hash(&eh, &entry).await, Ok(()));
    assert_matches!(
        check_new_entry_header(&fixt!(CreateLink).into()),
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
        check_update_reference(&eu, &NewEntryHeaderRef::from(&ec)),
        Ok(())
    );

    // Different app entry type
    ec.entry_type = et_app_2;

    assert_matches!(
        check_update_reference(&eu, &NewEntryHeaderRef::from(&ec)),
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::UpdateTypeMismatch(_, _)
        ))
    );

    // Different entry type
    eu.entry_type = et_cap;

    assert_matches!(
        check_update_reference(&eu, &NewEntryHeaderRef::from(&ec)),
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
    // Setup test data
    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_entry_type_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            origin_time: Timestamp::HOLOCHAIN_EPOCH,
            zomes: vec![TestWasm::EntryDefs.into()].into(),
        },
        vec![TestWasm::EntryDefs.into()],
    )
    .await
    .unwrap();
    let dna_hash = dna_file.dna_hash().to_owned().clone();
    let mut entry_def = fixt!(EntryDef);
    entry_def.visibility = EntryVisibility::Public;

    // Setup mock conductor
    let mut conductor_handle = MockConductorHandleT::new();
    // # No dna or entry def
    conductor_handle.expect_get_entry_def().return_const(None);
    conductor_handle.expect_get_dna_file().return_const(None);

    // ## Dna is missing
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeId out of range
    conductor_handle.checkpoint();
    conductor_handle.expect_get_entry_def().return_const(None);
    conductor_handle
        .expect_get_dna_file()
        .return_const(Some(dna_file.clone()));
    let aet = AppEntryType::new(0.into(), 1.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::ZomeId(_)
        ))
    );

    // ## EntryId is out of range
    let aet = AppEntryType::new(10.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&dna_hash, &aet, &conductor_handle).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::EntryDefId(_)
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
    let dna = fixt!(DnaFile);
    let dna_hash = dna.dna_hash().clone();
    let ds = MockDnaStore::single_dna(dna, 0, 0);
    let spaces = TestSpaces::new([dna_hash.clone()], RwShare::new(ds));
    let space = Arc::new(spaces.test_spaces[&dna_hash].space.clone());
    let vault = space.dht_env.clone();
    let keystore = test_keystore();
    let (tx, _rx) = TriggerSender::new();

    let private_entry = fixt!(Entry);
    let mut create = fixt!(Create);
    let author = keystore.new_sign_keypair_random().await.unwrap();
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Private);
    create.entry_type = EntryType::App(aet);
    create.entry_hash = EntryHash::with_data_sync(&private_entry);
    create.author = author.clone();
    let header = Header::Create(create);
    let signature = author.sign(&keystore, &header).await.unwrap();

    let shh =
        SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(header), signature);
    let el = Element::new(shh, Some(private_entry));

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_entry(el.clone()).await.unwrap();
    let num_ops: usize = fresh_reader_test(vault.clone(), |txn| {
        txn.query_row("SELECT COUNT(rowid) FROM DhtOp", [], |row| row.get(0))
            .unwrap()
    });
    assert_eq!(num_ops, 0);

    let ops_sender = IncomingDhtOpSender::new(space.clone(), tx.clone());
    ops_sender.send_store_element(el.clone()).await.unwrap();
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
    let author = fixt!(AgentPubKey);
    // Create a valid chain.
    let mut headers = vec![];
    headers.push(HeaderHashed::from_content_sync(Header::Dna(Dna {
        author: author.clone(),
        timestamp: Timestamp::from_micros(0),
        hash: fixt!(DnaHash),
    })));
    headers.push(HeaderHashed::from_content_sync(Header::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(1),
        header_seq: 1,
        prev_header: headers[0].to_hash(),
        entry_type: fixt!(EntryType),
        entry_hash: fixt!(EntryHash),
    })));
    headers.push(HeaderHashed::from_content_sync(Header::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(2),
        header_seq: 2,
        prev_header: headers[1].to_hash(),
        entry_type: fixt!(EntryType),
        entry_hash: fixt!(EntryHash),
    })));
    // Valid chain passes.
    validate_chain(headers.iter(), &None).expect("Valid chain");

    // Create a forked chain.
    let mut fork = headers.clone();
    fork.push(HeaderHashed::from_content_sync(Header::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(10),
        header_seq: 1,
        prev_header: headers[0].to_hash(),
        entry_type: fixt!(EntryType),
        entry_hash: fixt!(EntryHash),
    })));
    let err = validate_chain(fork.iter(), &None).expect_err("Forked chain");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::HashMismatch
        ))
    ));

    // Test a chain with the wrong seq.
    let mut wrong_seq = headers.clone();
    *wrong_seq[2].as_content_mut().header_seq_mut().unwrap() = 3;
    let err = validate_chain(wrong_seq.iter(), &None).expect_err("Wrong seq");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::InvalidSeq(_, _)
        ))
    ));

    // Test a wrong root gets rejected.
    let mut wrong_root = headers.clone();
    wrong_root[0] = HeaderHashed::from_content_sync(Header::Create(Create {
        author: author.clone(),
        timestamp: Timestamp::from_micros(0),
        header_seq: 0,
        prev_header: headers[0].to_hash(),
        entry_type: fixt!(EntryType),
        entry_hash: fixt!(EntryHash),
    }));
    let err = validate_chain(wrong_root.iter(), &None).expect_err("Wrong root");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::InvalidRoot
        ))
    ));

    // Test without dna at root gets rejected.
    let mut dna_not_at_root = headers.clone();
    dna_not_at_root.push(headers[0].clone());
    let err = validate_chain(dna_not_at_root.iter(), &None).expect_err("Dna not at root");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::InvalidRoot
        ))
    ));

    // Test if there is a existing head that a dna in the new chain is rejected.
    let err =
        validate_chain(headers.iter(), &Some((fixt!(HeaderHash), 0))).expect_err("Dna not at root");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::InvalidRoot
        ))
    ));

    // Check a sequence that is broken gets rejected.
    let mut wrong_seq = headers[1..].to_vec();
    *wrong_seq[0].as_content_mut().header_seq_mut().unwrap() = 3;
    *wrong_seq[1].as_content_mut().header_seq_mut().unwrap() = 4;
    let err = validate_chain(
        wrong_seq.iter(),
        &Some((wrong_seq[0].prev_header().unwrap().clone(), 0)),
    )
    .expect_err("Wrong seq");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::InvalidSeq(_, _)
        ))
    ));

    // Check the correct sequence gets accepted with a root.
    let correct_seq = headers[1..].to_vec();
    validate_chain(
        correct_seq.iter(),
        &Some((correct_seq[0].prev_header().unwrap().clone(), 0)),
    )
    .expect("Correct seq");

    let err = validate_chain(correct_seq.iter(), &Some((fixt!(HeaderHash), 0)))
        .expect_err("Hash is wrong");
    assert!(matches!(
        err,
        SysValidationError::ValidationOutcome(ValidationOutcome::PrevHeaderError(
            PrevHeaderError::HashMismatch
        ))
    ));
}
