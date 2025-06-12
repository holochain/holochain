use std::sync::Arc;

use fixt::fixt;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::DnaHashB64;
use holochain_p2p::HolochainPeerMetaStore;
use holochain_types::prelude::InstalledAppId;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::PeerMetaStore;
use kitsune2_api::Timestamp;
use kitsune2_api::Url;

use crate::sweettest::*;

#[tokio::test(flavor = "multi_thread")]
async fn agent_meta_info() {
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
    let timestamp: Timestamp = Timestamp::now();

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
            Some(timestamp),
        )
        .await
        .unwrap();

    // Since we're at it, we also want to test that the set_unresponsive()
    // method writes the data in a serde_json compatible format to the peer
    // meta store
    peer_meta_store1
        .mark_peer_unresponsive(url.clone(), timestamp, timestamp)
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
            Some(timestamp),
        )
        .await
        .unwrap();

    // Get the agent meta info for all spaces
    let agent_meta_info = conductor.agent_meta_info(url.clone(), None).await.unwrap();

    let expected_response = format!(
        r#"{{
  "{}": [
    {{
      "peer_url": "{}",
      "meta_key": "test:meta",
      "meta_value": "hello2",
      "expires_at": {:?}
    }}
  ],
  "{}": [
    {{
      "peer_url": "{}",
      "meta_key": "root:unresponsive",
      "meta_value": {:?},
      "expires_at": {:?}
    }},
    {{
      "peer_url": "{}",
      "meta_key": "test:meta",
      "meta_value": "hello",
      "expires_at": {:?}
    }}
  ]
}}"#,
        DnaHashB64::from(dna2.dna_hash().clone()),
        url.as_str(),
        timestamp.as_micros(),
        DnaHashB64::from(dna1.dna_hash().clone()),
        url.as_str(),
        timestamp.as_micros(),
        timestamp.as_micros(),
        url.as_str(),
        timestamp.as_micros(),
    );

    assert_eq!(agent_meta_info, expected_response);

    // Get the agent meta info for a selected space only
    let agent_meta_info2 = conductor
        .agent_meta_info(url.clone(), Some(vec![dna2.dna_hash().clone()]))
        .await
        .unwrap();

    let expected_response2 = format!(
        r#"{{
  "{}": [
    {{
      "peer_url": "{}",
      "meta_key": "test:meta",
      "meta_value": "hello2",
      "expires_at": {:?}
    }}
  ]
}}"#,
        DnaHashB64::from(dna2.dna_hash().clone()),
        url.as_str(),
        timestamp.as_micros()
    );

    assert_eq!(agent_meta_info2, expected_response2);

    // Try to get agent meta info for a non-existent space. Should
    // throw an error.
    let res = conductor
        .agent_meta_info(url.clone(), Some(vec![fixt!(DnaHash)]))
        .await;

    assert!(res.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn app_agent_meta_info() {
    holochain_trace::test_run();

    let (dna1, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let (dna2, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductor = SweetConductor::from_standard_config().await;
    let app_id1: InstalledAppId = "app1".into();
    conductor
        .setup_app(&app_id1, &[dna1.clone()])
        .await
        .unwrap();

    let app_id2: InstalledAppId = "app2".into();
    conductor
        .setup_app(&app_id2, &[dna2.clone()])
        .await
        .unwrap();

    let url = Url::from_str("ws://test.com:80/test-url").unwrap();
    let timestamp: Timestamp = Timestamp::now();

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
            Some(timestamp),
        )
        .await
        .unwrap();

    // Get the agent meta info for all spaces of app1. Should only return
    // info for the space of app1
    let agent_meta_info = conductor
        .app_agent_meta_info(&app_id1, url.clone(), None)
        .await
        .unwrap();

    println!("Got agent_meta_info: {}", agent_meta_info);

    let expected_response = format!(
        r#"{{
  "{}": [
    {{
      "peer_url": "{}",
      "meta_key": "test:meta",
      "meta_value": "hello",
      "expires_at": {:?}
    }}
  ]
}}"#,
        DnaHashB64::from(dna1.dna_hash().clone()),
        url.as_str(),
        timestamp.as_micros()
    );

    assert_eq!(agent_meta_info, expected_response);
}
