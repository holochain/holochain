use super::*;
use crate::conductor::api::error::ConductorApiError;
use crate::conductor::api::MockCellConductorApi;
use crate::meta_mock;
use ::fixt::prelude::*;
use error::SysValidationError;

use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::db::DbRead;
use holochain_sqlite::test_utils::test_cell_env;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::Header;
use matches::assert_matches;
use observability;
use std::convert::TryFrom;

#[tokio::test(flavor = "multi_thread")]
async fn verify_header_signature_test() {
    let keystore = holochain_sqlite::test_utils::test_keystore();
    let author = fake_agent_pubkey_1();
    let mut header = fixt!(CreateLink);
    header.author = author.clone();
    let header = Header::CreateLink(header);
    let real_signature = author.sign(&keystore, &header).await.unwrap();
    let wrong_signature = Signature([1_u8; 64]);

    assert_matches!(
        verify_header_signature(&wrong_signature, &header).await,
        Ok(false)
    );

    assert_matches!(
        verify_header_signature(&real_signature, &header).await,
        Ok(true)
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
    let tmp = test_cell_env();
    let env: DbRead = tmp.env().into();
    // Test data
    let activity_return = vec![fixt!(HeaderHash)];

    // Empty store not dna
    let header = fixt!(CreateLink);
    let mut metadata = meta_mock!();
    metadata.expect_env().return_const(env.clone());

    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &metadata).await,
        Ok(())
    );
    let header = fixt!(Dna);
    let mut metadata = meta_mock!(expect_get_activity);
    metadata.expect_env().return_const(env.clone());
    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &metadata).await,
        Ok(())
    );

    let mut metadata = meta_mock!(expect_get_activity, activity_return);
    metadata.expect_env().return_const(env);
    assert_matches!(
        check_valid_if_dna(&header.clone().into(), &metadata).await,
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidRoot)
        ))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_previous_timestamp() {
    let mut header = fixt!(CreateLink);
    let mut prev_header = fixt!(CreateLink);
    header.timestamp = timestamp::now().into();
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
        Err(
            SysValidationError::ValidationOutcome(
                ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
            ),
        )
    );

    prev_header.header_seq = 3;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Err(
            SysValidationError::ValidationOutcome(
                ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
            ),
        )
    );

    header.header_seq = 0;
    prev_header.header_seq = 0;
    assert_matches!(
        check_prev_seq(&header.clone().into(), &prev_header.clone().into()),
        Err(
            SysValidationError::ValidationOutcome(
                ValidationOutcome::PrevHeaderError(PrevHeaderError::InvalidSeq(_, _)),
            ),
        )
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
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::NotNewEntry(_)))
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
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::UpdateTypeMismatch(_, _)))
    );

    // Different entry type
    eu.entry_type = et_cap;

    assert_matches!(
        check_update_reference(&eu, &NewEntryHeaderRef::from(&ec)),
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::UpdateTypeMismatch(_, _)))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_link_tag_size_test() {
    let tiny = LinkTag(vec![0; 1]);
    let bytes = (0..401).map(|_| 0u8).into_iter().collect::<Vec<_>>();
    let huge = LinkTag(bytes);
    assert_matches!(check_tag_size(&tiny), Ok(()));

    assert_matches!(
        check_tag_size(&huge),
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::TagTooLarge(_, _)))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn check_app_entry_type_test() {
    observability::test_run().ok();
    // Setup test data
    let dna_file = DnaFile::new(
        DnaDef {
            name: "app_entry_type_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
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
    let mut conductor_api = MockCellConductorApi::new();
    conductor_api.expect_cell_id().return_const(fixt!(CellId));
    // # No dna or entry def
    conductor_api.expect_sync_get_entry_def().return_const(None);
    conductor_api.expect_sync_get_dna().return_const(None);
    conductor_api
        .expect_sync_get_this_dna()
        .returning(move || Err(ConductorApiError::DnaMissing(dna_hash.clone())));

    // ## Dna is missing
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&aet, &conductor_api).await,
        Err(SysValidationError::ConductorApiError(e))
        if matches!(*e, ConductorApiError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeId out of range
    conductor_api.checkpoint();
    conductor_api.expect_sync_get_entry_def().return_const(None);
    conductor_api
        .expect_sync_get_dna()
        .return_const(Some(dna_file.clone()));
    conductor_api
        .expect_sync_get_this_dna()
        .returning(move || Ok(dna_file.clone()));
    let aet = AppEntryType::new(0.into(), 1.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&aet, &conductor_api).await,
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::ZomeId(_)))
    );

    // ## EntryId is out of range
    let aet = AppEntryType::new(10.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&aet, &conductor_api).await,
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::EntryDefId(_)))
    );

    // ## EntryId is in range for dna
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(check_app_entry_type(&aet, &conductor_api).await, Ok(_));
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Private);
    assert_matches!(
        check_app_entry_type(&aet, &conductor_api).await,
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::EntryVisibility(_)))
    );

    // # Add an entry def to the buffer
    conductor_api
        .expect_sync_get_entry_def()
        .return_const(Some(entry_def));

    // ## Can get the entry from the entry def
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(check_app_entry_type(&aet, &conductor_api).await, Ok(_));
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
