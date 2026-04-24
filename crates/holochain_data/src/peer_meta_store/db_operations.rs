//! Database handle operations for the peer metadata store.
//!
//! Provides [`DbRead`] and [`DbWrite`] impls for querying and mutating the peer metadata store.

use super::{inner, PeerMetaEntry};
use crate::handles::{DbRead, DbWrite};
use crate::kind::PeerMetaStore;

impl DbRead<PeerMetaStore> {
    /// Get the value for a specific peer URL and key, if it exists and has not expired.
    pub async fn get(&self, peer_url: &str, meta_key: &str) -> sqlx::Result<Option<Vec<u8>>> {
        inner::get(self.pool(), peer_url, meta_key).await
    }

    /// Get all non-expired peer URLs and values for a given metadata key.
    pub async fn get_all_by_key(&self, meta_key: &str) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        inner::get_all_by_key(self.pool(), meta_key).await
    }

    /// Get all non-expired metadata entries for a given peer URL.
    pub async fn get_all_by_url(&self, peer_url: &str) -> sqlx::Result<Vec<PeerMetaEntry>> {
        inner::get_all_by_url(self.pool(), peer_url).await
    }
}

impl DbWrite<PeerMetaStore> {
    /// Insert or replace a peer metadata entry.
    pub async fn put(
        &self,
        peer_url: &str,
        meta_key: &str,
        meta_value: &[u8],
        expires_at: Option<i64>,
    ) -> sqlx::Result<()> {
        inner::put(self.pool(), peer_url, meta_key, meta_value, expires_at).await
    }

    /// Delete a specific peer metadata entry.
    pub async fn delete(&self, peer_url: &str, meta_key: &str) -> sqlx::Result<()> {
        inner::delete(self.pool(), peer_url, meta_key).await
    }

    /// Delete all expired entries. Returns the number of rows removed.
    pub async fn prune(&self) -> sqlx::Result<u64> {
        inner::prune(self.pool()).await
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::PeerMetaStore;
    use crate::peer_meta_store::PeerMetaEntry;
    use crate::test_open_db;
    use holo_hash::DnaHash;
    use std::sync::Arc;

    fn test_db_id() -> PeerMetaStore {
        PeerMetaStore::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    #[tokio::test]
    async fn get_returns_none_for_missing_entry() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn get_returns_none_for_expired_entry() {
        let db = test_open_db(test_db_id()).await.unwrap();

        // expires_at set to Unix epoch
        db.put("wss://peer.example", "foo", b"123", Some(0))
            .await
            .unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn get_data_that_was_put() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, Some(b"123".to_vec()));
    }

    #[tokio::test]
    async fn put_replaces_existing_entry() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer.example", "foo", b"200", None)
            .await
            .unwrap();

        let entries = db.as_ref().get_all_by_key("foo").await.unwrap();

        // Only one entry should exist with the new value
        assert_eq!(
            entries,
            [("wss://peer.example".to_string(), b"200".to_vec())]
        );
    }

    #[tokio::test]
    async fn delete_removes_valid_entry() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();
        db.delete("wss://peer.example", "foo").await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn delete_succeeds_with_invalid_entry() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();
        let result = db.delete("wss://peer.example", "bar").await;

        assert!(result.is_ok());

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, Some(b"123".to_vec()));
    }

    #[tokio::test]
    async fn get_all_by_key_returns_empty_if_no_entries() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let entries = db.as_ref().get_all_by_key("foo").await.unwrap();
        assert!(entries.is_empty(),);
    }

    #[tokio::test]
    async fn get_all_by_key_returns_only_that_key() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer.example", "bar", b"999", None)
            .await
            .unwrap();

        let entries = db.as_ref().get_all_by_key("foo").await.unwrap();
        assert_eq!(
            entries,
            [("wss://peer.example".to_string(), b"100".to_vec())]
        );
    }

    #[tokio::test]
    async fn get_all_by_key_returns_for_all_peers() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer-a.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer-b.example", "foo", b"200", None)
            .await
            .unwrap();

        let entries = db.as_ref().get_all_by_key("foo").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&("wss://peer-a.example".to_string(), b"100".to_vec())));
        assert!(entries.contains(&("wss://peer-b.example".to_string(), b"200".to_vec())));
    }

    #[tokio::test]
    async fn get_all_by_key_returns_only_non_expired() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer-a.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer-b.example", "foo", b"200", Some(0))
            .await
            .unwrap();

        let entries = db.as_ref().get_all_by_key("foo").await.unwrap();
        assert_eq!(
            entries,
            [("wss://peer-a.example".to_string(), b"100".to_vec())]
        );
    }

    #[tokio::test]
    async fn get_all_by_url_returns_empty_if_no_entries() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert!(entries.is_empty(),);
    }

    #[tokio::test]
    async fn get_all_by_url_returns_only_that_url() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer-a.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer-b.example", "foo", b"999", None)
            .await
            .unwrap();

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer-a.example")
            .await
            .unwrap();
        assert_eq!(
            entries,
            [PeerMetaEntry {
                meta_key: "foo".to_string(),
                meta_value: b"100".to_vec(),
                expires_at: None,
            }]
        );
    }

    #[tokio::test]
    async fn get_all_by_url_returns_for_that_url() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer.example", "bar", b"200", None)
            .await
            .unwrap();

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&PeerMetaEntry {
            meta_key: "foo".to_string(),
            meta_value: b"100".to_vec(),
            expires_at: None,
        }));
        assert!(entries.contains(&PeerMetaEntry {
            meta_key: "bar".to_string(),
            meta_value: b"200".to_vec(),
            expires_at: None,
        }));
    }

    #[tokio::test]
    async fn get_all_by_url_returns_only_non_expired() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"100", None)
            .await
            .unwrap();
        db.put("wss://peer.example", "bar", b"200", Some(0))
            .await
            .unwrap();

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(
            entries,
            [PeerMetaEntry {
                meta_key: "foo".to_string(),
                meta_value: b"100".to_vec(),
                expires_at: None,
            },]
        );
    }

    #[tokio::test]
    async fn prune_returns_zero_if_no_entries_to_prune() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();
        db.put("wss://peer.example", "bar", b"234", None)
            .await
            .unwrap();

        let pruned = db.prune().await.unwrap();
        assert_eq!(pruned, 0);

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&PeerMetaEntry {
            meta_key: "foo".to_string(),
            meta_value: b"123".to_vec(),
            expires_at: None,
        }));
        assert!(entries.contains(&PeerMetaEntry {
            meta_key: "bar".to_string(),
            meta_value: b"234".to_vec(),
            expires_at: None,
        }));
    }

    #[tokio::test]
    async fn prune_removes_expired_entries() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "expired", b"123", Some(1))
            .await
            .unwrap();
        db.put("wss://peer.example", "live", b"234", None)
            .await
            .unwrap();

        let pruned = db.prune().await.unwrap();
        assert_eq!(pruned, 1);

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(
            entries,
            [PeerMetaEntry {
                meta_key: "live".to_string(),
                meta_value: b"234".to_vec(),
                expires_at: None,
            },]
        );
    }
}
