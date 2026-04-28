//! Operations for the peer metadata store database.
//!
//! This module provides operations that can be performed on the peer metadata store database, which
//! holds arbitrary key-value metadata about peers with optional expiry times.

mod inner;

pub mod db_operations;
pub mod tx_operations;

/// A single entry read from the peer metadata store database.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PeerMetaEntry {
    /// The key of the metadata in the store.
    pub meta_key: String,
    /// The raw metadata value.
    pub meta_value: Vec<u8>,
    /// Expiry time in seconds since the Unix epoch, [`None`] if the entry never expires.
    pub expires_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use crate::kind::PeerMetaStore;
    use crate::test_open_db;
    use holo_hash::DnaHash;
    use std::sync::Arc;

    fn test_db_id() -> PeerMetaStore {
        PeerMetaStore::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    #[tokio::test]
    async fn schema_created() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(db.pool())
                .await
                .unwrap();

        assert!(tables.contains(&"peer_meta".to_string()));
    }

    #[tokio::test]
    async fn expires_at_index_is_partial() {
        let db = test_open_db(test_db_id()).await.unwrap();

        let partial: Option<i64> = sqlx::query_scalar(
            "SELECT partial FROM pragma_index_list('peer_meta') WHERE name = 'expires_at_idx'",
        )
        .fetch_optional(db.pool())
        .await
        .unwrap();

        assert_eq!(
            partial,
            Some(1),
            "expires_at_idx should exist and be a partial index"
        );
    }
}
