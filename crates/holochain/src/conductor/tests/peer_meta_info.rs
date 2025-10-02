use crate::sweettest::*;
use fixt::fixt;
use holo_hash::fixt::DnaHashFixturator;
use holochain_p2p::HolochainPeerMetaStore;
use holochain_types::prelude::InstalledAppId;
use holochain_types::prelude::Timestamp as HTimestamp;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::PeerMetaStore;
use kitsune2_api::Timestamp;
use kitsune2_api::Url;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn peer_meta_info() {
    holochain_trace::test_run();

    // We want deterministic dna hashes to get the JSON output in a deterministic
    // oder
    let (dna1, _, _) = SweetDnaFile::from_test_wasms(
        "deterministic seed".into(),
        vec![TestWasm::Create],
        Default::default(),
    )
    .await;
    let (dna2, _, _) = SweetDnaFile::from_test_wasms(
        "deterministic seed".into(),
        vec![TestWasm::WhoAmI],
        Default::default(),
    )
    .await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let app_id: InstalledAppId = "app".into();
    conductor
        .setup_app(&app_id, &[dna1.clone(), dna2.clone()])
        .await
        .unwrap();

    let url = Url::from_str("ws://test.com:80/test-url").unwrap();
    let ktimestamp = Timestamp::now();
    let htimestamp = HTimestamp(ktimestamp.as_micros());

    // Write a 1-2 peer meta entries into each space's peer meta store
    let db1 = conductor
        .spaces
        .peer_meta_store_db(dna1.dna_hash())
        .unwrap();

    let peer_meta_store1 = Arc::new(HolochainPeerMetaStore::create(db1.clone()).await.unwrap());
    peer_meta_store1
        .put(
            url.clone(),
            "test:meta".into(),
            serde_json::to_vec("hello").unwrap().into(),
            Some(ktimestamp),
        )
        .await
        .unwrap();

    // Since we're at it, we also want to test that the set_unresponsive()
    // method writes the data in a serde_json compatible format to the peer
    // meta store
    peer_meta_store1
        .set_unresponsive(url.clone(), ktimestamp, ktimestamp)
        .await
        .unwrap();

    let db2 = conductor
        .spaces
        .peer_meta_store_db(dna2.dna_hash())
        .unwrap();

    let peer_meta_store2 = Arc::new(HolochainPeerMetaStore::create(db2.clone()).await.unwrap());
    peer_meta_store2
        .put(
            url.clone(),
            "test:meta".into(),
            serde_json::to_vec("hello2").unwrap().into(),
            Some(ktimestamp),
        )
        .await
        .unwrap();

    // Get the agent meta info for all spaces
    let response = conductor.peer_meta_info(url.clone(), None).await.unwrap();

    assert_eq!(response.len(), 2);

    let meta_infos1 = response
        .get(dna1.dna_hash())
        .expect("No entry for dna 2 found.");

    assert_eq!(meta_infos1.len(), 2);

    let peer_meta_info11 = meta_infos1.get("root:unresponsive").unwrap();
    assert_eq!(peer_meta_info11.meta_value, ktimestamp.as_micros());
    assert_eq!(peer_meta_info11.expires_at, Some(htimestamp));

    let peer_meta_info12 = meta_infos1.get("test:meta").unwrap();
    assert_eq!(
        peer_meta_info12.meta_value,
        serde_json::Value::String("hello".into())
    );
    assert_eq!(peer_meta_info12.expires_at, Some(htimestamp));

    let meta_infos2 = response
        .get(dna2.dna_hash())
        .expect("No value found for dna 2.");

    assert_eq!(meta_infos2.len(), 1);

    let peer_meta_info2 = meta_infos2.get("test:meta").unwrap();
    assert_eq!(
        peer_meta_info2.meta_value,
        serde_json::Value::String("hello2".into())
    );
    assert_eq!(peer_meta_info2.expires_at, Some(htimestamp));

    // Get the agent meta info for a selected space only
    let response2 = conductor
        .peer_meta_info(url.clone(), Some(vec![dna2.dna_hash().clone()]))
        .await
        .unwrap();

    assert_eq!(response2.len(), 1);

    let meta_infos2 = response
        .get(dna2.dna_hash())
        .expect("No value found for dna 2.");

    assert_eq!(meta_infos2.len(), 1);

    let peer_meta_info2 = meta_infos2.get("test:meta").unwrap();
    assert_eq!(
        peer_meta_info2.meta_value,
        serde_json::Value::String("hello2".into())
    );
    assert_eq!(peer_meta_info2.expires_at, Some(htimestamp));

    // Try to get agent meta info for a non-existent space. Should
    // throw an error.
    let res = conductor
        .peer_meta_info(url.clone(), Some(vec![fixt!(DnaHash)]))
        .await;

    assert!(res.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn app_peer_meta_info() {
    holochain_trace::test_run();

    let (dna1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductor = SweetConductor::from_standard_config().await;
    let app_id1: InstalledAppId = "app1".into();
    conductor
        .setup_app(&app_id1, std::slice::from_ref(&dna1))
        .await
        .unwrap();

    let app_id2: InstalledAppId = "app2".into();
    conductor
        .setup_app(&app_id2, std::slice::from_ref(&dna2))
        .await
        .unwrap();

    let url = Url::from_str("ws://test.com:80/test-url").unwrap();
    let ktimestamp: Timestamp = Timestamp::now();
    let htimestamp = HTimestamp(ktimestamp.as_micros());

    // Write a peer meta entry into app1's peer meta store
    let db1 = conductor
        .spaces
        .peer_meta_store_db(dna1.dna_hash())
        .unwrap();

    let peer_meta_store1 = Arc::new(HolochainPeerMetaStore::create(db1.clone()).await.unwrap());
    peer_meta_store1
        .put(
            url.clone(),
            "test:meta".into(),
            serde_json::to_vec("hello").unwrap().into(),
            Some(ktimestamp),
        )
        .await
        .unwrap();

    // Get the agent meta info for all spaces of app1. Should only return
    // info for the space of app1
    let response = conductor
        .app_peer_meta_info(&app_id1, url.clone(), None)
        .await
        .unwrap();
    assert_eq!(response.len(), 1);

    let meta_infos = response
        .get(dna1.dna_hash())
        .expect("No value found for dna");
    assert_eq!(meta_infos.len(), 1);

    let peer_meta_info = meta_infos.get("test:meta").unwrap();
    assert_eq!(
        peer_meta_info.meta_value,
        serde_json::Value::String("hello".into())
    );
    assert_eq!(peer_meta_info.expires_at, Some(htimestamp));
}
