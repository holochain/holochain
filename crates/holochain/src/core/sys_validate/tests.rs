use super::*;
use crate::{conductor::api::MockCellConductorApi, meta_mock};
use ::fixt::prelude::*;
use error::SysValidationError;
use holo_hash::fixt::*;
use holochain_keystore::AgentPubKeyExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{env::EnvironmentRead, test_utils::test_cell_env};
use holochain_types::{
    dna::{DnaDef, DnaFile},
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    fixt::*,
    observability,
    test_utils::{fake_agent_pubkey_1, fake_header_hash},
    Timestamp,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{header::InitZomesComplete, Header};
use matches::assert_matches;
use std::convert::{TryFrom, TryInto};

async fn test_gen(ts: Timestamp, seq: u32, prev: HeaderHash) -> Element {
    let keystore = holochain_state::test_utils::test_keystore();

    let header = InitZomesComplete {
        author: fake_agent_pubkey_1(),
        timestamp: ts.into(),
        header_seq: seq,
        prev_header: prev,
    };

    let hashed = HeaderHashed::from_content_sync(header.into());
    let signed = SignedHeaderHashed::new(&keystore, hashed).await.unwrap();
    Element::new(signed, None)
}

#[tokio::test(threaded_scheduler)]
async fn valid_headers_validate() {
    let first = test_gen(
        "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
        12,
        fake_header_hash(1),
    )
    .await;
    let second = test_gen(
        "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
        13,
        first.header_address().clone(),
    )
    .await;

    sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first))
        .await
        .unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn invalid_hash_headers_dont_validate() {
    let first = test_gen(
        "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
        12,
        fake_header_hash(1),
    )
    .await;
    let second = test_gen(
        "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
        13,
        fake_header_hash(2),
    )
    .await;

    matches::assert_matches!(
        sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
        Err(SourceChainError::InvalidPreviousHeader(_))
    );
}

#[tokio::test(threaded_scheduler)]
async fn invalid_timestamp_headers_dont_validate() {
    let first = test_gen(
        "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
        12,
        fake_header_hash(1),
    )
    .await;
    let second = test_gen(
        "2020-05-05T19:16:04.166431045Z".try_into().unwrap(),
        13,
        first.header_address().clone(),
    )
    .await;

    matches::assert_matches!(
        sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
        Err(SourceChainError::InvalidPreviousHeader(_))
    );
}

#[tokio::test(threaded_scheduler)]
async fn invalid_seq_headers_dont_validate() {
    let first = test_gen(
        "2020-05-05T19:16:04.266431045Z".try_into().unwrap(),
        12,
        fake_header_hash(1),
    )
    .await;
    let second = test_gen(
        "2020-05-05T19:16:04.366431045Z".try_into().unwrap(),
        14,
        first.header_address().clone(),
    )
    .await;

    matches::assert_matches!(
        sys_validate_element(&fake_agent_pubkey_1(), &second, Some(&first)).await,
        Err(SourceChainError::InvalidPreviousHeader(_))
    );
}

#[tokio::test(threaded_scheduler)]
async fn verify_header_signature_test() {
    let keystore = holochain_state::test_utils::test_keystore();
    let author = fake_agent_pubkey_1();
    let mut header = fixt!(CreateLink);
    header.author = author.clone();
    let header = Header::CreateLink(header);
    let real_signature = author.sign(&keystore, &header).await.unwrap();
    let wrong_signature = Signature(vec![1; 64]);

    assert_matches!(
        verify_header_signature(&wrong_signature, &header).await,
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::VerifySignature(_, _)))
    );

    assert_matches!(
        verify_header_signature(&real_signature, &header).await,
        Ok(())
    );
}

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
async fn check_valid_if_dna_test() {
    let env: EnvironmentRead = test_cell_env().env.into();
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

#[tokio::test(threaded_scheduler)]
async fn check_prev_header_in_metadata_test() {
    let env: EnvironmentRead = test_cell_env().env.into();
    // Test data
    let mut header_fixt = HeaderHashFixturator::new(Predictable);
    let prev_header_hash = header_fixt.next().unwrap();
    let author = fixt!(AgentPubKey);
    let activity_return = vec![prev_header_hash.clone()];
    let mut metadata = meta_mock!(expect_get_activity, activity_return, {
        let author = author.clone();
        move |a| *a == author
    });

    metadata.expect_env().return_const(env);

    // Previous header on this hash
    assert_matches!(
        check_prev_header_in_metadata(&author, &prev_header_hash, &metadata).await,
        Ok(())
    );

    // No previous header on this hash
    assert_matches!(
        check_prev_header_in_metadata(&author, &header_fixt.next().unwrap(), &metadata).await,
        Err(SysValidationError::ValidationOutcome(ValidationOutcome::NotHoldingDep(_)))
    );
}

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
async fn check_entry_hash_test() {
    let mut ec = fixt!(CreateEntry);
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

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
async fn check_update_reference_test() {
    let mut ec = fixt!(CreateEntry);
    let mut eu = fixt!(UpdateEntry);
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

#[tokio::test(threaded_scheduler)]
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

#[tokio::test(threaded_scheduler)]
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
    let mut entry_def = fixt!(EntryDef);
    entry_def.visibility = EntryVisibility::Public;

    // Setup mock conductor
    let mut conductor_api = MockCellConductorApi::new();
    conductor_api.expect_cell_id().return_const(fixt!(CellId));
    // # No dna or entry def
    conductor_api.expect_sync_get_entry_def().return_const(None);
    conductor_api.expect_sync_get_this_dna().return_const(None);

    // ## Dna is missing
    let aet = AppEntryType::new(0.into(), 0.into(), EntryVisibility::Public);
    assert_matches!(
        check_app_entry_type(&aet, &conductor_api).await,
        Err(SysValidationError::DnaMissing(_))
    );

    // # Dna but no entry def in buffer
    // ## ZomeId out of range
    conductor_api.checkpoint();
    conductor_api.expect_sync_get_entry_def().return_const(None);
    conductor_api
        .expect_sync_get_this_dna()
        .return_const(Some(dna_file));
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

#[tokio::test(threaded_scheduler)]
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
