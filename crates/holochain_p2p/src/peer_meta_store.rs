use bytes::Bytes;
use futures::future::BoxFuture;
use holochain_sqlite::db::DbWrite;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::DbKindPeerMetaStore;
use holochain_sqlite::rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
use holochain_sqlite::rusqlite::{named_params, ToSql};
use holochain_sqlite::sql::sql_peer_meta_store;
use kitsune2_api::{BoxFut, K2Error, K2Result, PeerMetaStore, Timestamp, Url};
use std::sync::Arc;

/// Key prefix for items at the root level of the peer meta store.
pub const KEY_PREFIX_ROOT: &str = "root";

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

struct BytesSql(Bytes);

impl ToSql for BytesSql {
    #[inline]
    fn to_sql(&self) -> holochain_sqlite::rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(&self.0[..]))
    }
}

impl FromSql for BytesSql {
    #[inline]
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(BytesSql(Bytes::copy_from_slice(value.as_blob()?)))
    }
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

    /// Note that expired peer URLs are pruned at a fixed interval, not precisely when the expiry elapsed.
    fn mark_peer_unresponsive(
        &self,
        peer: Url,
        expiry: Timestamp,
        when: Timestamp,
    ) -> BoxFuture<'_, K2Result<()>> {
        Box::pin(async move {
            self.put(
                peer.clone(),
                format!("{KEY_PREFIX_ROOT}:unresponsive"),
                rmp_serde::to_vec(&when).expect("expected Timestamp").into(),
                Some(expiry),
            )
            .await?;
            Ok(())
        })
    }

    fn get_unresponsive_url(&self, peer: Url) -> BoxFuture<'_, K2Result<Option<Timestamp>>> {
        Box::pin(async move {
            self.get(peer, format!("{KEY_PREFIX_ROOT}:unresponsive"))
                .await
                .map(|maybe_value| {
                    maybe_value
                        .map(|value| rmp_serde::from_slice(&value).expect("expected Timestamp"))
                })
        })
    }
}
