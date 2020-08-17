use super::*;
use holochain_types::{
    element::{SignedHeaderHashed, SignedHeaderHashedExt},
    test_utils::{fake_agent_pubkey_1, fake_header_hash},
    Timestamp,
};
use holochain_zome_types::header::InitZomesComplete;
use std::convert::TryInto;

async fn test_gen(ts: Timestamp, seq: u32, prev: HeaderHash) -> Element {
    let keystore = holochain_state::test_utils::test_keystore();

    let header = InitZomesComplete {
        author: fake_agent_pubkey_1(),
        timestamp: ts.into(),
        header_seq: seq,
        prev_header: prev,
    };

    let hashed = HeaderHashed::from_content(header.into()).await;
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
