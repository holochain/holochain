use bytes::Bytes;
use futures::future::BoxFuture;
use holochain_sqlite::db::DbWrite;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::DbKindPeerMetaStore;
use holochain_sqlite::rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
use holochain_sqlite::rusqlite::{named_params, ToSql};
use holochain_sqlite::sql::sql_peer_meta_store;
use kitsune2_api::{K2Error, K2Result, PeerMetaStore, Timestamp, Url};

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
        // Prune any expired entries on startup
        db.write_async(|txn| -> DatabaseResult<()> {
            txn.execute(sql_peer_meta_store::PRUNE, [])?;

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
            db.write_async(move |txn| -> DatabaseResult<Option<Bytes>> {
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
}
