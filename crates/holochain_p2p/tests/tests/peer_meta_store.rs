use bytes::Bytes;
use holo_hash::DnaHash;
use holochain_p2p::HolochainPeerMetaStore;
use holochain_sqlite::db::{DbKindPeerMetaStore, DbWrite, ReadAccess};
use holochain_sqlite::error::DatabaseResult;
use kitsune2_api::{PeerMetaStore, Timestamp, Url};
use std::sync::Arc;

#[tokio::test]
async fn peer_meta_crd() {
    let db = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(DnaHash::from_raw_36(
        vec![0xdb; 36],
    ))))
    .unwrap();

    let store = HolochainPeerMetaStore::create(db).await.unwrap();

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
async fn prune_on_create() {
    let db = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(DnaHash::from_raw_36(
        vec![0xdb; 36],
    ))))
    .unwrap();

    {
        let store = HolochainPeerMetaStore::create(db.clone()).await.unwrap();

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

        let count = db
            .read_async(|txn| -> DatabaseResult<u32> {
                let count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                    row.get::<_, u32>(0)
                })?;
                Ok(count)
            })
            .await
            .unwrap();

        assert_eq!(1, count);
    }

    // Setting up a new store should clear expired values
    HolochainPeerMetaStore::create(db.clone()).await.unwrap();

    let count = db
        .read_async(|txn| -> DatabaseResult<u32> {
            let count = txn.query_row("SELECT COUNT(*) FROM peer_meta", [], |row| {
                row.get::<_, u32>(0)
            })?;
            Ok(count)
        })
        .await
        .unwrap();

    assert_eq!(0, count);
}

#[tokio::test]
async fn mark_peer_unresponsive_in_peer_meta_store() {
    let db = DbWrite::test_in_mem(DbKindPeerMetaStore(Arc::new(DnaHash::from_raw_36(
        vec![0x0a; 36],
    ))))
    .unwrap();
    let store = Arc::new(HolochainPeerMetaStore::create(db.clone()).await.unwrap());
    let peer_url = Url::from_str("ws://test:80/1").unwrap();
    let when_peer_marked_unresponsive = store.get_unresponsive_url(peer_url.clone()).await.unwrap();
    assert!(when_peer_marked_unresponsive.is_none());
    let when = Timestamp::now();
    store
        .mark_peer_unresponsive(peer_url.clone(), Timestamp::now(), when)
        .await
        .unwrap();
    let when_peer_marked_unresponsive = store.get_unresponsive_url(peer_url).await.unwrap();
    assert_eq!(when_peer_marked_unresponsive, Some(when));
}
