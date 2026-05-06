use bytes::Bytes;
use holo_hash::DnaHash;
use holochain_data::kind::PeerMetaStore;
use holochain_p2p::HolochainPeerMetaStore;
use kitsune2_api::{PeerMetaStore as _, Timestamp, Url, KEY_PREFIX_ROOT, META_KEY_UNRESPONSIVE};
use std::sync::Arc;
use std::time::Duration;

fn test_db_id() -> PeerMetaStore {
    PeerMetaStore::new(Arc::new(DnaHash::from_raw_36(vec![0xdb; 36])))
}

#[tokio::test]
async fn peer_meta_crd() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();

    let store = HolochainPeerMetaStore::create(
        holochain_state::peer_metadata_store::PeerMetaStore::new(db),
    )
    .await
    .unwrap();

    let peer_url = Url::from_str("ws://test:80/1").unwrap();
    let key = "test".to_string();

    store
        .put(
            peer_url.clone(),
            key.clone(),
            Bytes::from_static("test".as_bytes()),
            None,
        )
        .await
        .unwrap();

    let value = store.get(peer_url.clone(), key.clone()).await.unwrap();

    assert!(value.is_some());
    assert_eq!(Bytes::from_static("test".as_bytes()), value.unwrap());

    store.delete(peer_url.clone(), key.clone()).await.unwrap();

    let value = store.get(peer_url, key).await.unwrap();

    assert!(value.is_none());
}

#[tokio::test]
async fn get_all_urls_by_key() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();
    let store = HolochainPeerMetaStore::create(
        holochain_state::peer_metadata_store::PeerMetaStore::new(db),
    )
    .await
    .unwrap();

    // Insert 2 URLs with a key into the store.
    let key = "key".to_string();
    let url_1 = Url::from_str("ws://test:80/1").unwrap();
    let url_2 = Url::from_str("ws://test:80/2").unwrap();
    let value_1 = Bytes::from_static(b"value_1");
    let value_2 = Bytes::from_static(b"value_2");
    store
        .put(url_1.clone(), key.clone(), value_1.clone(), None)
        .await
        .unwrap();
    store
        .put(url_2.clone(), key.clone(), value_2.clone(), None)
        .await
        .unwrap();

    // Insert another URL with a different key into the store.
    let unrelated_url = Url::from_str("ws://unrelated:80").unwrap();
    let unrelated_key = "unrelated".to_string();
    store
        .put(
            unrelated_url.clone(),
            unrelated_key.clone(),
            Bytes::new(),
            None,
        )
        .await
        .unwrap();

    let all_related_urls = store.get_all_by_key(key).await.unwrap();
    assert_eq!(all_related_urls.len(), 2);
    assert_eq!(*all_related_urls.get(&url_1).unwrap(), value_1);
    assert_eq!(*all_related_urls.get(&url_2).unwrap(), value_2);
    assert!(!all_related_urls.contains_key(&unrelated_url));
}

#[tokio::test]
async fn get_all_unresponsive_urls_by_key() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();
    let store = HolochainPeerMetaStore::create(
        holochain_state::peer_metadata_store::PeerMetaStore::new(db),
    )
    .await
    .unwrap();

    // Insert 2 unresponsive URLs into store.
    let unresponsive_url_1 = Url::from_str("ws://test:80/1").unwrap();
    let unresponsive_url_2 = Url::from_str("ws://test:80/2").unwrap();
    store
        .set_unresponsive(
            unresponsive_url_1.clone(),
            Timestamp::from_micros(i64::MAX),
            Timestamp::now(),
        )
        .await
        .unwrap();
    store
        .set_unresponsive(
            unresponsive_url_2.clone(),
            Timestamp::from_micros(i64::MAX),
            Timestamp::now(),
        )
        .await
        .unwrap();

    // Insert another, unrelated URL into store.
    let unrelated_url = Url::from_str("ws://unrelated:80").unwrap();
    let unrelated_key = "unrelated".to_string();
    store
        .put(
            unrelated_url.clone(),
            unrelated_key.clone(),
            Bytes::new(),
            None,
        )
        .await
        .unwrap();

    let all_unresponsive_urls = store
        .get_all_by_key(format!("{KEY_PREFIX_ROOT}:{META_KEY_UNRESPONSIVE}"))
        .await
        .unwrap();
    assert_eq!(all_unresponsive_urls.len(), 2);
    assert!(all_unresponsive_urls.contains_key(&unresponsive_url_1));
    assert!(all_unresponsive_urls.contains_key(&unresponsive_url_2));
    assert!(!all_unresponsive_urls.contains_key(&unrelated_url));
}

#[tokio::test]
async fn prune_on_create() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();

    {
        let store = HolochainPeerMetaStore::create(
            holochain_state::peer_metadata_store::PeerMetaStore::new(db.clone()),
        )
        .await
        .unwrap();

        let peer_url = Url::from_str("ws://test:80/1").unwrap();
        let key = "test".to_string();

        store
            .put(
                peer_url,
                key,
                Bytes::from_static("test".as_bytes()),
                Some(Timestamp::from_micros(0)),
            )
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM peer_meta")
            .fetch_one(db.pool())
            .await
            .unwrap();

        assert_eq!(1, count);
    }

    // Setting up a new store should clear expired values
    HolochainPeerMetaStore::create(holochain_state::peer_metadata_store::PeerMetaStore::new(
        db.clone(),
    ))
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM peer_meta")
        .fetch_one(db.pool())
        .await
        .unwrap();

    assert_eq!(0, count);
}

#[tokio::test]
async fn set_peer_unresponsive_in_peer_meta_store() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();
    let store = Arc::new(
        HolochainPeerMetaStore::create(holochain_state::peer_metadata_store::PeerMetaStore::new(
            db.clone(),
        ))
        .await
        .unwrap(),
    );
    let peer_url = Url::from_str("ws://test:80/1").unwrap();
    let when_peer_set_unresponsive = store.get_unresponsive(peer_url.clone()).await.unwrap();
    assert!(when_peer_set_unresponsive.is_none());
    let when = Timestamp::now();
    store
        .set_unresponsive(peer_url.clone(), Timestamp::from_micros(i64::MAX), when)
        .await
        .unwrap();
    let when_peer_marked_unresponsive = store.get_unresponsive(peer_url).await.unwrap();
    assert_eq!(when_peer_marked_unresponsive, Some(when));
}

#[tokio::test]
#[cfg_attr(
    not(feature = "transport-iroh"),
    ignore = "requires Iroh transport for stability"
)]
async fn unresponsive_peers_are_removed_from_store_after_expiry() {
    let db = holochain_data::test_open_db(test_db_id()).await.unwrap();
    let store = Arc::new(
        HolochainPeerMetaStore::create(holochain_state::peer_metadata_store::PeerMetaStore::new(
            db.clone(),
        ))
        .await
        .unwrap(),
    );

    let peer_url = Url::from_str("ws://test:80/1").unwrap();
    // Expiry time needs to be more than 1 second in the future as we might be on the boundary of
    // this second.
    let expiry = Timestamp::now() + Duration::from_secs(2);
    let when = Timestamp::now();
    store
        .set_unresponsive(peer_url.clone(), expiry, when)
        .await
        .unwrap();

    // Waiting until the next pruning, but before the expiry, to make sure expiry is respected.
    tokio::time::sleep(Duration::from_millis(10)).await;
    let when_peer_marked_unresponsive = store.get_unresponsive(peer_url.clone()).await.unwrap();
    assert_eq!(when_peer_marked_unresponsive, Some(when));

    // Wait until expiry time.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Manually trigger pruning by recreating the store (which prunes on startup).
    drop(store);
    let store = Arc::new(
        HolochainPeerMetaStore::create(holochain_state::peer_metadata_store::PeerMetaStore::new(
            db.clone(),
        ))
        .await
        .unwrap(),
    );

    let when_peer_set_unresponsive = store.get_unresponsive(peer_url).await.unwrap();
    assert!(when_peer_set_unresponsive.is_none());
}
