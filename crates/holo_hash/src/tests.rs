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
struct TestHeader(String);

impl_hashable_content!(TestDhtOp, DhtOp);
impl_hashable_content!(TestHeader, Header);

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
#[ignore = "TODO"]
fn check_serialized_bytes() {
    let h: HeaderHash =
        HeaderHash::try_from("uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();

    let h: SerializedBytes = h.try_into().unwrap();

    assert_eq!(
            "{\"type\":\"HeaderHash\",\"hash\":[88,43,0,130,130,164,145,252,50,36,8,37,143,125,49,95,241,139,45,95,183,5,123,133,203,141,250,107,100,170,165,193,48,200,28,230]}",
            &format!("{:?}", h),
        );

    let h = HeaderHash::try_from(h).unwrap();

    assert_eq!(
        "HeaderHash(uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );
}

#[test]
fn holo_hash_parse() {
    let h = DnaHash::try_from("uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(3_860_645_936 as u32, h.get_loc());
    assert_eq!(
        "DnaHash(uhC0kWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = NetIdHash::try_from("uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(3_860_645_936, h.get_loc());
    assert_eq!(
        "NetIdHash(uhCIkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = HeaderHash::try_from("uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(3_860_645_936, h.get_loc());
    assert_eq!(
        "HeaderHash(uhCkkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = EntryHash::try_from("uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(3_860_645_936, h.get_loc());
    assert_eq!(
        "EntryHash(uhCEkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );

    let h = DhtOpHash::try_from("uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm").unwrap();
    assert_eq!(3_860_645_936, h.get_loc());
    assert_eq!(
        "DhtOpHash(uhCQkWCsAgoKkkfwyJAglj30xX_GLLV-3BXuFy436a2SqpcEwyBzm)",
        &format!("{:?}", h),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_as_bytes() {
    tokio::task::spawn(async move {
        let hash = vec![0xdb; 32];
        let hash: &[u8] = &hash;
        let agent_id = HeaderHash::from_raw_32(hash.to_vec());
        assert_eq!(hash, agent_id.get_bytes());
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_prehash_display() {
    tokio::task::spawn(async move {
        let agent_id = HeaderHash::from_raw_32(vec![0xdb; 32]);
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
    let agent_id: HeaderHash =
        HeaderHash::try_from("uhCkkdwAAuHr_AKFTzF2vjvVzlkWTOxdAhqZ00jcBe9GZQs77BSjQ").unwrap();
    assert_eq!(3_492_283_899, agent_id.get_loc());
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_debug() {
    tokio::task::spawn(async move {
        let agent_id = HeaderHash::with_data_sync(&TestHeader("hi".to_string()));
        assert_eq!(
            "HeaderHash(uhCkkdwAAuHr_AKFTzF2vjvVzlkWTOxdAhqZ00jcBe9GZQs77BSjQ)",
            &format!("{:?}", agent_id),
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_id_display() {
    tokio::task::spawn(async move {
        let agent_id = HeaderHash::with_data_sync(&TestHeader("hi".to_string()));
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
        let agent_id = HeaderHash::with_data_sync(&TestHeader("hi".to_string()));
        assert_eq!(3_492_283_899, agent_id.get_loc());
    })
    .await
    .unwrap();
}
