#![cfg(test)]

use crate::{HashableContent, HoloHashed};
use holochain_serialized_bytes::prelude::*;
use std::convert::TryInto;

/// test struct
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct TestDhtOp {
    /// string
    pub s: String,
    /// integer
    pub i: i64,
}

type TestDhtOpHashed = HoloHashed<TestDhtOp>;

/// test struct
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct TestAction(String);

impl_hashable_content!(TestDhtOp, DhtOp);
impl_hashable_content!(TestAction, Action);

#[tokio::test(flavor = "multi_thread")]
async fn check_hashed_type() {
    let my_type = TestDhtOp {
        s: "test".to_string(),
        i: 42,
    };

    let my_type_hashed = TestDhtOpHashed::from_content_sync(my_type);

    assert_eq!(
        "uhCQkQFRMcbVVfPJ5AbAv0HJq0geatTakGEEj5rpv_Dp0pjmJob3P",
        my_type_hashed.as_hash().to_string(),
    );
}


#[test]
fn holo_hash_parse() {
    let expected_loc = 3_860_645_936_u32;
    let h = DnaHash::try_from("uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "DnaHash(uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = NetIdHash::try_from("uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "NetIdHash(uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = ActionHash::try_from("uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "ActionHash(uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = EntryHash::try_from("uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "EntryHash(uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = DhtOpHash::try_from("uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "DhtOpHash(uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = ExternalHash::try_from("uhC8kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(expected_loc, h.get_loc());
    assert_eq!(
        "ExternalHash(uhC8kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_as_bytes() {
    tokio::task::spawn(async move {
        let hash = vec![0xdb; 32];
        let hash: &[u8] = &hash;
        let agent_id = ActionHash::from_raw_32(hash.to_vec());
        assert_eq!(hash, agent_id.get_bytes());
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_prehash_display() {
    tokio::task::spawn(async move {
        let agent_id = ActionHash::from_raw_32(vec![0xdb; 32]);
        assert_eq!(
            "uhCkk29vb29vb29vb29vb29vb29vb29vb29vb29vb29vb29uTp5Iv",
            &format!("{}", agent_id.to_string()),
        );
    })
    .await
    .unwrap();
}

#[test]
fn agent_id_try_parse() {
    let agent_id: ActionHash =
        ActionHash::try_from("uhCkkdwAAuHr_AKFTzF2vjvVzlkWTOxdAhqZ00jcBe9GZQs77BSjQ").unwrap();
    assert_eq!(3_492_283_899, agent_id.get_loc());
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_debug() {
    tokio::task::spawn(async move {
        let agent_id = TestAction("hi".to_string()).to_hash();
        assert_eq!(
            "ActionHash(uhCkkdwAAuHr_AKFTzF2vjvVzlkWTOxdAhqZ00jcBe9GZQs77BSjQ)",
            &format!("{:?}", agent_id),
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_display() {
    tokio::task::spawn(async move {
        let agent_id = TestAction("hi".to_string()).to_hash();
        assert_eq!(
            "uhCkkdwAAuHr_AKFTzF2vjvVzlkWTOxdAhqZ00jcBe9GZQs77BSjQ",
            &format!("{}", agent_id.to_string()),
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_loc() {
    tokio::task::spawn(async move {
        let agent_id = TestAction("hi".to_string()).to_hash();
        assert_eq!(3_492_283_899, agent_id.get_loc());
    })
    .await
    .unwrap();
}
