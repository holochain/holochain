//! Transaction-scoped operations for the peer metadata store.
//!
//! Provides [`TxRead`] and [`TxWrite`] impls for querying and mutating the peer metadata store.

use super::{inner, PeerMetaEntry};
use crate::handles::{TxRead, TxWrite};
use crate::kind::PeerMetaStore;

impl TxRead<PeerMetaStore> {
    /// Get the value for a specific peer URL and key, if it exists and has not expired.
    pub async fn get(&mut self, peer_url: &str, meta_key: &str) -> sqlx::Result<Option<Vec<u8>>> {
        inner::get(self.conn_mut(), peer_url, meta_key).await
    }

    /// Get all non-expired peer URLs and values for a given metadata key.
    pub async fn get_all_by_key(&mut self, meta_key: &str) -> sqlx::Result<Vec<(String, Vec<u8>)>> {
        inner::get_all_by_key(self.conn_mut(), meta_key).await
    }

    /// Get all non-expired metadata entries for a given peer URL.
    pub async fn get_all_by_url(&mut self, peer_url: &str) -> sqlx::Result<Vec<PeerMetaEntry>> {
        inner::get_all_by_url(self.conn_mut(), peer_url).await
    }
}

impl TxWrite<PeerMetaStore> {
    /// Insert or replace a peer metadata entry.
    ///
    /// `expires_at` is seconds since the Unix epoch.
    pub async fn put(
        &mut self,
        peer_url: &str,
        meta_key: &str,
        meta_value: &[u8],
        expires_at_secs: Option<i64>,
    ) -> sqlx::Result<()> {
        inner::put(
            self.conn_mut(),
            peer_url,
            meta_key,
            meta_value,
            expires_at_secs,
        )
        .await
    }

    /// Delete a specific peer metadata entry.
    pub async fn delete(&mut self, peer_url: &str, meta_key: &str) -> sqlx::Result<()> {
        inner::delete(self.conn_mut(), peer_url, meta_key).await
    }

    /// Delete all expired entries. Returns the number of rows removed.
    pub async fn prune(&mut self) -> sqlx::Result<u64> {
        inner::prune(self.conn_mut()).await
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
    async fn tx_write_commit_persists_puts() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.put("wss://peer.example", "foo", b"42", None)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, Some(b"42".to_vec()));
    }

    #[tokio::test]
    async fn tx_write_rollback_discards_puts() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.put("wss://peer.example", "foo", b"42", None)
            .await
            .unwrap();
        tx.rollback().await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn tx_write_commit_persists_deletes() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"42", None)
            .await
            .unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.delete("wss://peer.example", "foo").await.unwrap();
        tx.commit().await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn tx_write_rollback_discards_deletes() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "foo", b"42", None)
            .await
            .unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.delete("wss://peer.example", "foo").await.unwrap();
        tx.rollback().await.unwrap();

        let value = db.as_ref().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, Some(b"42".to_vec()));
    }

    #[tokio::test]
    async fn tx_write_commit_persists_prunes() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "expired", b"old", Some(1))
            .await
            .unwrap();
        db.put("wss://peer.example", "live", b"current", None)
            .await
            .unwrap();

        let mut tx = db.begin().await.unwrap();
        let pruned = tx.prune().await.unwrap();
        assert_eq!(pruned, 1);
        tx.commit().await.unwrap();

        let entries = db
            .as_ref()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(
            entries,
            [PeerMetaEntry {
                meta_key: "live".to_string(),
                meta_value: b"current".to_vec(),
                expires_at: None,
            }]
        );
    }

    #[tokio::test]
    async fn tx_write_rollback_discards_prunes() {
        let db = test_open_db(test_db_id()).await.unwrap();

        db.put("wss://peer.example", "expired", b"old", Some(1))
            .await
            .unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.prune().await.unwrap();
        tx.rollback().await.unwrap();

        // Can't use get here because it filters expired entries.
        let all: Vec<_> = sqlx::query_scalar::<_, String>(
            "SELECT meta_key FROM peer_meta WHERE peer_url = 'wss://peer.example'",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert!(all.contains(&"expired".to_string()));
    }

    #[tokio::test]
    async fn tx_read_get_sees_own_puts() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.put("wss://peer.example", "foo", b"42", None)
            .await
            .unwrap();

        let value = tx.as_mut().get("wss://peer.example", "foo").await.unwrap();
        assert_eq!(value, Some(b"42".to_vec()));

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn tx_read_get_all_by_key_sees_own_puts() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.put("wss://peer-a.example", "foo", b"1", None)
            .await
            .unwrap();
        tx.put("wss://peer-b.example", "foo", b"2", None)
            .await
            .unwrap();

        let entries = tx.as_mut().get_all_by_key("foo").await.unwrap();
        assert_eq!(entries.len(), 2);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn tx_read_get_all_by_url_sees_own_puts() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let mut tx = db.begin().await.unwrap();
        tx.put("wss://peer.example", "foo", b"1", None)
            .await
            .unwrap();
        tx.put("wss://peer.example", "bar", b"2", None)
            .await
            .unwrap();

        let entries = tx
            .as_mut()
            .get_all_by_url("wss://peer.example")
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);

        tx.rollback().await.unwrap();
    }
}
