use bytes::Bytes;
use futures::future::BoxFuture;
use holochain_state::peer_metadata_store::PeerMetaStore;
use kitsune2_api::{BoxFut, K2Error, K2Result, Timestamp, Url};
use std::collections::HashMap;
use std::sync::Arc;

/// Holochain implementation of the Kitsune2 [kitsune2_api::PeerMetaStoreFactory].
pub struct HolochainPeerMetaStoreFactory {
    /// The database connection getter.
    pub getter: crate::GetDbPeerMeta,
}

impl std::fmt::Debug for HolochainPeerMetaStoreFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainPeerMetaStoreFactory").finish()
    }
}

impl kitsune2_api::PeerMetaStoreFactory for HolochainPeerMetaStoreFactory {
    fn default_config(&self, _config: &mut kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &kitsune2_api::Config) -> kitsune2_api::K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<kitsune2_api::Builder>,
        space: kitsune2_api::SpaceId,
    ) -> BoxFut<'static, kitsune2_api::K2Result<kitsune2_api::DynPeerMetaStore>> {
        let getter = self.getter.clone();
        Box::pin(async move {
            let store = getter(holo_hash::DnaHash::from_k2_space(&space))
                .await
                .map_err(|err| {
                    kitsune2_api::K2Error::other_src("failed to get peer_meta_store", err)
                })?;
            let peer_meta_store: kitsune2_api::DynPeerMetaStore =
                Arc::new(HolochainPeerMetaStore::create(store).await.map_err(|err| {
                    K2Error::other_src("failed to connect to peer store database", err)
                })?);

            Ok(peer_meta_store)
        })
    }
}

/// Holochain implementation of a Kitsune2 [kitsune2_api::PeerMetaStore].
#[derive(Debug)]
pub struct HolochainPeerMetaStore {
    db: PeerMetaStore,
}

impl HolochainPeerMetaStore {
    /// Create a new [HolochainPeerMetaStore] from a [`PeerMetaStore`].
    pub async fn create(db: PeerMetaStore) -> K2Result<Self> {
        // Prune any expired entries on startup.
        let prune_count = db
            .prune()
            .await
            .map_err(|e| K2Error::other_src("Failed to prune peer meta store on startup", e))?;
        tracing::debug!("pruned {prune_count} rows from peer meta store");

        Ok(Self { db })
    }
}

impl kitsune2_api::PeerMetaStore for HolochainPeerMetaStore {
    fn put(
        &self,
        peer: Url,
        key: String,
        value: Bytes,
        expiry: Option<Timestamp>,
    ) -> BoxFuture<'_, K2Result<()>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.put(
                peer.as_str(),
                &key,
                &value,
                expiry.map(|expiry| expiry.as_micros() / 1_000_000),
            )
            .await
            .map_err(|e| K2Error::other_src("Failed to put peer meta", e))
        })
    }

    fn get(&self, peer: Url, key: String) -> BoxFuture<'_, K2Result<Option<Bytes>>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.as_read()
                .get(peer.as_str(), &key)
                .await
                .map(|value| value.map(Bytes::from))
                .map_err(|e| K2Error::other_src("Failed to get peer meta", e))
        })
    }

    fn get_all_by_key(&self, key: String) -> BoxFuture<'_, K2Result<HashMap<Url, Bytes>>> {
        let db = self.db.clone();
        Box::pin(async move {
            let entries = db
                .as_read()
                .get_all_by_key(&key)
                .await
                .map_err(|e| K2Error::other_src("Failed to get all values", e))?;
            let mut map = HashMap::with_capacity(entries.len());
            for (url_str, value) in entries {
                let url = Url::from_str(url_str)
                    .map_err(|e| K2Error::other_src("Invalid peer URL in peer meta store", e))?;
                map.insert(url, Bytes::from(value));
            }
            Ok(map)
        })
    }

    fn delete(&self, peer: Url, key: String) -> BoxFuture<'_, K2Result<()>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.delete(peer.as_str(), &key)
                .await
                .map_err(|e| K2Error::other_src("Failed to delete peer meta", e))
        })
    }
}
