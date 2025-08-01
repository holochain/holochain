use bytes::Bytes;
use futures::future::BoxFuture;
use holochain_sqlite::db::DbWrite;
use holochain_sqlite::error::{DatabaseError, DatabaseResult};
use holochain_sqlite::helpers::BytesSql;
use holochain_sqlite::prelude::DbKindPeerMetaStore;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::sql::sql_peer_meta_store;
use kitsune2_api::{BoxFut, K2Error, K2Result, PeerMetaStore, Timestamp, Url};
use std::collections::HashMap;
use std::sync::Arc;

/// Holochain implementation of the Kitsune2 [kitsune2_api::OpStoreFactory].
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
            let db = getter(holo_hash::DnaHash::from_k2_space(&space))
                .await
                .map_err(|err| {
                    kitsune2_api::K2Error::other_src("failed to get peer_meta_store db", err)
                })?;
            let peer_meta_store: kitsune2_api::DynPeerMetaStore =
                Arc::new(HolochainPeerMetaStore::create(db).await.map_err(|err| {
                    K2Error::other_src("failed to connect to peer store database", err)
                })?);

            Ok(peer_meta_store)
        })
    }
}

/// Holochain implementation of a Kitsune2 [PeerMetaStore].
#[derive(Debug)]
pub struct HolochainPeerMetaStore {
    db: DbWrite<DbKindPeerMetaStore>,
}

impl HolochainPeerMetaStore {
    /// Create a new [HolochainPeerMetaStore] from a database handle.
    pub async fn create(db: DbWrite<DbKindPeerMetaStore>) -> DatabaseResult<Self> {
        // Prune any expired entries on startup.
        db.write_async(|txn| -> DatabaseResult<()> {
            let prune_count = txn.execute(sql_peer_meta_store::PRUNE, [])?;
            tracing::debug!("pruned {prune_count} rows from meta peer store");
            Ok(())
        })
        .await?;

        Ok(Self { db })
    }
}

impl PeerMetaStore for HolochainPeerMetaStore {
    fn put(
        &self,
        peer: Url,
        key: String,
        value: Bytes,
        expiry: Option<Timestamp>,
    ) -> BoxFuture<'_, K2Result<()>> {
        let db = self.db.clone();

        Box::pin(async move {
            db.write_async(move |txn| -> DatabaseResult<()> {
                txn.execute(
                    sql_peer_meta_store::INSERT,
                    named_params! {
                        ":peer_url": peer.as_str(),
                        ":meta_key": key,
                        ":meta_value": BytesSql(value),
                        ":expires_at": expiry.map(|e| e.as_micros()),
                    },
                )?;

                Ok(())
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to put peer meta", e))
        })
    }

    fn get(&self, peer: Url, key: String) -> BoxFuture<'_, K2Result<Option<Bytes>>> {
        let db = self.db.clone();

        Box::pin(async move {
            db.read_async(move |txn| -> DatabaseResult<Option<Bytes>> {
                let value = match txn.query_row(
                    sql_peer_meta_store::GET,
                    named_params! {
                        ":peer_url": peer.as_str(),
                        ":meta_key": key,
                    },
                    |row| row.get::<_, BytesSql>(0),
                ) {
                    Ok(value) => Some(value.0),
                    Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(e) => return Err(e.into()),
                };

                Ok(value)
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to get peer meta", e))
        })
    }

    fn get_all_by_key(&self, key: String) -> BoxFuture<'_, K2Result<HashMap<Url, Bytes>>> {
        let db = self.db.clone();
        Box::pin(async move {
            db.read_async(move |txn| -> DatabaseResult<HashMap<Url, Bytes>> {
                let mut stmt = txn.prepare(sql_peer_meta_store::GET_ALL_BY_KEY)?;
                let mut rows = stmt.query(named_params! {":meta_key": key})?;
                let mut all_values = HashMap::new();
                while let Some(row) = rows.next()? {
                    let peer_url = Url::from_str(row.get::<_, String>(0)?)
                        .map_err(|err| DatabaseError::Other(err.into()))?;
                    let value = row.get::<_, BytesSql>(1)?;
                    all_values.insert(peer_url, value.0);
                }
                Ok(all_values)
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to get all values", e))
        })
    }

    fn delete(&self, peer: Url, key: String) -> BoxFuture<'_, K2Result<()>> {
        let db = self.db.clone();

        Box::pin(async move {
            db.write_async(move |txn| -> DatabaseResult<()> {
                txn.execute(
                    sql_peer_meta_store::DELETE,
                    named_params! {
                        ":peer_url": peer.as_str(),
                        ":meta_key": key,
                    },
                )?;

                Ok(())
            })
            .await
            .map_err(|e| K2Error::other_src("Failed to delete peer meta", e))
        })
    }
}
