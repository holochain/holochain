//! Module to wrap the Peer Metadata Store database from the [`holochain_data`] crate.

use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;

pub use holochain_data::peer_meta_store::PeerMetaEntry;

/// A type to hold the Peer Metadata Store database from [`holochain_data`].
#[derive(Debug, Clone)]
pub struct PeerMetaStore<Db = holochain_data::DbWrite<holochain_data::kind::PeerMetaStore>> {
    db: Db,
}

/// A read-only view of the Peer Metadata Store.
pub type PeerMetaStoreRead =
    PeerMetaStore<holochain_data::DbRead<holochain_data::kind::PeerMetaStore>>;

impl<Db> PeerMetaStore<Db> {
    /// Create a new [`PeerMetaStore`] from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl PeerMetaStore<holochain_data::DbRead<holochain_data::kind::PeerMetaStore>> {
    /// Get the value for a specific peer URL and key, if it exists and has not expired.
    pub async fn get(&self, peer_url: &str, meta_key: &str) -> StateQueryResult<Option<Vec<u8>>> {
        self.db
            .get(peer_url, meta_key)
            .await
            .map_err(StateQueryError::from)
    }

    /// Get all non-expired peer URLs and values for a given metadata key.
    pub async fn get_all_by_key(&self, meta_key: &str) -> StateQueryResult<Vec<(String, Vec<u8>)>> {
        self.db
            .get_all_by_key(meta_key)
            .await
            .map_err(StateQueryError::from)
    }

    /// Get all non-expired metadata entries for a given peer URL.
    pub async fn get_all_by_url(&self, peer_url: &str) -> StateQueryResult<Vec<PeerMetaEntry>> {
        self.db
            .get_all_by_url(peer_url)
            .await
            .map_err(StateQueryError::from)
    }
}

impl PeerMetaStore<holochain_data::DbWrite<holochain_data::kind::PeerMetaStore>> {
    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> PeerMetaStoreRead {
        PeerMetaStoreRead::new(self.db.as_ref().clone())
    }

    /// Convert this writable store into a read-only store.
    pub fn into_read(self) -> PeerMetaStoreRead {
        PeerMetaStoreRead::new(self.db.into())
    }

    /// Insert or replace a peer metadata entry.
    ///
    /// `expires_at` is seconds since the Unix epoch.
    pub async fn put(
        &self,
        peer_url: &str,
        meta_key: &str,
        meta_value: &[u8],
        expires_at_secs: Option<i64>,
    ) -> StateMutationResult<()> {
        self.db
            .put(peer_url, meta_key, meta_value, expires_at_secs)
            .await
            .map_err(StateMutationError::from)
    }

    /// Delete a specific peer metadata entry.
    pub async fn delete(&self, peer_url: &str, meta_key: &str) -> StateMutationResult<()> {
        self.db
            .delete(peer_url, meta_key)
            .await
            .map_err(StateMutationError::from)
    }

    /// Delete all expired entries. Returns the number of rows removed.
    pub async fn prune(&self) -> StateMutationResult<u64> {
        self.db.prune().await.map_err(StateMutationError::from)
    }
}

impl From<PeerMetaStore<holochain_data::DbWrite<holochain_data::kind::PeerMetaStore>>>
    for PeerMetaStoreRead
{
    fn from(
        store: PeerMetaStore<holochain_data::DbWrite<holochain_data::kind::PeerMetaStore>>,
    ) -> Self {
        store.into_read()
    }
}

#[cfg(feature = "test_utils")]
impl<Db> PeerMetaStore<Db>
where
    Db: AsRef<holochain_data::DbRead<holochain_data::kind::PeerMetaStore>>,
{
    /// Get a reference to the raw database handle for testing purposes.
    pub fn raw_db_read(&self) -> &holochain_data::DbRead<holochain_data::kind::PeerMetaStore> {
        self.db.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use holo_hash::DnaHash;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn peer_meta_store_round_trip() -> StateQueryResult<()> {
        holochain_trace::test_run();

        let tempdir = tempfile::tempdir().unwrap();
        let db = holochain_data::open_db(
            tempdir.path(),
            holochain_data::kind::PeerMetaStore::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36]))),
            holochain_data::HolochainDataConfig {
                key: None,
                sync_level: holochain_data::DbSyncLevel::Off,
                max_readers: 8,
            },
        )
        .await
        .map_err(StateQueryError::from)?;

        let store = PeerMetaStore::new(db);
        store
            .put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();
        let read = store
            .as_read()
            .get("wss://peer.example", "foo")
            .await?
            .unwrap();
        assert_eq!(read, b"123");

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn peer_meta_store_downgrade() -> StateQueryResult<()> {
        holochain_trace::test_run();

        let tempdir = tempfile::tempdir().unwrap();
        let db = holochain_data::open_db(
            tempdir.path(),
            holochain_data::kind::PeerMetaStore::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36]))),
            holochain_data::HolochainDataConfig {
                key: None,
                sync_level: holochain_data::DbSyncLevel::Off,
                max_readers: 8,
            },
        )
        .await
        .map_err(StateQueryError::from)?;

        let store = PeerMetaStore::new(db);

        // Write with the writable store
        store
            .put("wss://peer.example", "foo", b"123", None)
            .await
            .unwrap();

        // Downgrade to read-only store using as_read()
        let read_store = store.as_read();
        let read = read_store.get("wss://peer.example", "foo").await?.unwrap();
        assert_eq!(read, b"123");

        // Test into_read() conversion
        let read_store2: PeerMetaStoreRead = store.into();
        let read2 = read_store2.get("wss://peer.example", "foo").await?.unwrap();
        assert_eq!(read2, b"123");

        Ok(())
    }
}
