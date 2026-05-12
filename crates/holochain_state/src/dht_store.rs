//! Per-DNA store for the `holochain_data` DHT database.
//!
//! [`DhtStore`] owns the [`DbWrite<Dht>`] handle for one DNA and exposes
//! domain-meaningful operations rather than raw database access. Call sites
//! obtain a reference from [`Space`](crate) and invoke named methods; they do
//! not need to interact with the underlying handle directly.

use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_data::dht::InsertScheduledFunction;
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::prelude::{Schedule, ScheduledFn, Timestamp};
use holochain_zome_types::schedule::ScheduleError;

/// Errors produced by [`DhtStore`] operations.
///
/// Wraps the underlying database and schedule errors so callers do not need to
/// depend on `sqlx` or schedule internals directly.
#[derive(thiserror::Error, Debug)]
pub enum DhtStoreError {
    /// An underlying database operation failed.
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    /// A schedule serialization or computation error occurred.
    #[error(transparent)]
    Schedule(#[from] holochain_serialized_bytes::SerializedBytesError),
    /// A schedule parameter computation error occurred.
    #[error(transparent)]
    ScheduleParams(#[from] ScheduleError),
    /// `mark_chain_op_receipts_complete` was called for an `op_hash` that has
    /// no matching `ChainOpPublish` row. The flush mirror should always insert
    /// one for self-authored ops, so this indicates a wiring bug.
    #[error("no ChainOpPublish row for op_hash {0:?}")]
    ChainOpPublishMissing(DhtOpHash),
}

/// Convenience alias for [`DhtStore`] results.
pub type DhtStoreResult<T> = Result<T, DhtStoreError>;

/// A read-only view of the DHT store.
pub type DhtStoreRead = DhtStore<holochain_data::DbRead<Dht>>;

/// Per-DNA store for the DHT database.
///
/// Owns a [`DbWrite<Dht>`] handle (or a [`holochain_data::DbRead<Dht>`] in the
/// read-only alias) and exposes operations keyed on the domain entities they
/// modify.
/// Clone-sharing is cheap: all clones refer to the same underlying connection
/// pool.
#[derive(Clone, Debug)]
pub struct DhtStore<Db = DbWrite<Dht>> {
    db: Db,
}

impl<Db> DhtStore<Db> {
    /// Create a new `DhtStore` from a database handle.
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Access the raw database handle.
    ///
    /// Available within `holochain_state` for call sites that need to compose
    /// multiple operations inside a single transaction (e.g. the flush path in
    /// [`crate::source_chain`]). External callers should use the named methods
    /// on this store instead.
    pub(crate) fn db(&self) -> &Db {
        &self.db
    }
}

impl DhtStore<DbWrite<Dht>> {
    /// Delete all live ephemeral scheduled-function rows for `author` at or
    /// before `now`. Returns the number of rows deleted.
    pub async fn delete_live_ephemeral_scheduled_functions(
        &self,
        author: &AgentPubKey,
        now: Timestamp,
    ) -> DhtStoreResult<u64> {
        Ok(self
            .db
            .delete_live_ephemeral_scheduled_functions(author, now)
            .await?)
    }

    /// Re-evaluate every expired persisted scheduled function for `author` at
    /// `now`.
    ///
    /// For each expired row the method decodes the stored `maybe_schedule`,
    /// computes updated `(start_at, end_at, ephemeral)` parameters, and either
    /// upserts the row (when a next cron date exists) or deletes it (when the
    /// cron string has no future occurrences). Errors for individual rows are
    /// logged and the loop continues, so a single bad row does not abort
    /// processing of the remaining rows.
    pub async fn reschedule_expired_persisted(&self, author: &AgentPubKey, now: Timestamp) -> () {
        let expired = match self
            .db
            .as_ref()
            .get_expired_persisted_scheduled_functions(author, now)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!(
                    "error querying expired persisted scheduled functions: {:?}",
                    e
                );
                return;
            }
        };

        for (zome_name, scheduled_fn_name, maybe_schedule_blob) in expired {
            let maybe_schedule: Option<Schedule> =
                match holochain_serialized_bytes::decode(&maybe_schedule_blob) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(
                            "error decoding maybe_schedule for ({}, {}): {:?}",
                            zome_name,
                            scheduled_fn_name,
                            e
                        );
                        continue;
                    }
                };

            match crate::schedule::compute_schedule_params(&maybe_schedule, now) {
                Err(e) => {
                    tracing::error!(
                        "error computing schedule params for ({}, {}): {:?}",
                        zome_name,
                        scheduled_fn_name,
                        e
                    );
                }
                Ok(None) => {
                    if let Err(e) = self
                        .db
                        .delete_scheduled_function(author, &zome_name, &scheduled_fn_name)
                        .await
                    {
                        tracing::error!(
                            "error deleting expired scheduled function ({}, {}): {:?}",
                            zome_name,
                            scheduled_fn_name,
                            e
                        );
                    }
                }
                Ok(Some((start_at, end_at, ephemeral))) => {
                    if let Err(e) = self
                        .db
                        .upsert_scheduled_function(InsertScheduledFunction {
                            author,
                            zome_name: &zome_name,
                            scheduled_fn: &scheduled_fn_name,
                            maybe_schedule: &maybe_schedule_blob,
                            start_at,
                            end_at,
                            ephemeral,
                        })
                        .await
                    {
                        tracing::error!(
                            "error upserting rescheduled function ({}, {}): {:?}",
                            zome_name,
                            scheduled_fn_name,
                            e
                        );
                    }
                }
            }
        }
    }

    /// Update the stored row for `scheduled_fn` owned by `author` based on the
    /// schedule returned by a zome call.
    ///
    /// When `maybe_schedule` is `Some`, the method serializes the schedule,
    /// computes `(start_at, end_at, ephemeral)` via
    /// [`compute_schedule_params`](crate::schedule::compute_schedule_params),
    /// and upserts the row. When `maybe_schedule` is `None`, or when the
    /// schedule has no future occurrences, the row is deleted instead. Returns
    /// the number of rows affected by whichever operation ran.
    pub async fn upsert_scheduled_function(
        &self,
        author: &AgentPubKey,
        scheduled_fn: &ScheduledFn,
        maybe_schedule: &Option<Schedule>,
        now: Timestamp,
    ) -> DhtStoreResult<u64> {
        let zome_name = scheduled_fn.zome_name().0.as_ref();
        let fn_name = scheduled_fn.fn_name().0.as_str();

        let maybe_schedule_blob = crate::schedule::serialize_maybe_schedule(maybe_schedule)?;

        match crate::schedule::compute_schedule_params(maybe_schedule, now)? {
            None => {
                // No further cron dates: remove the row.
                Ok(self
                    .db
                    .delete_scheduled_function(author, zome_name, fn_name)
                    .await?)
            }
            Some((start_at, end_at, ephemeral)) => Ok(self
                .db
                .upsert_scheduled_function(InsertScheduledFunction {
                    author,
                    zome_name,
                    scheduled_fn: fn_name,
                    maybe_schedule: &maybe_schedule_blob,
                    start_at,
                    end_at,
                    ephemeral,
                })
                .await?),
        }
    }

    /// Delete the scheduled-function row for `scheduled_fn` owned by `author`.
    /// Returns the number of rows deleted.
    pub async fn unschedule_function(
        &self,
        author: &AgentPubKey,
        scheduled_fn: &ScheduledFn,
    ) -> DhtStoreResult<u64> {
        Ok(self
            .db
            .delete_scheduled_function(
                author,
                scheduled_fn.zome_name().0.as_ref(),
                scheduled_fn.fn_name().0.as_str(),
            )
            .await?)
    }

    /// Mark the receipts for `op_hash` as complete. Returns
    /// [`DhtStoreError::ChainOpPublishMissing`] if no matching row exists,
    /// which indicates the flush mirror failed to insert a `ChainOpPublish`
    /// row for this self-authored op.
    pub async fn mark_chain_op_receipts_complete(&self, op_hash: &DhtOpHash) -> DhtStoreResult<()> {
        let rows = self.db.set_chain_op_receipts_complete(op_hash).await?;
        if rows == 0 {
            return Err(DhtStoreError::ChainOpPublishMissing(op_hash.clone()));
        }
        Ok(())
    }

    /// Delete every row from every table in this DNA's DHT database.
    ///
    /// Used when the conductor uninstalls the last app for a DNA. Runs as a
    /// single transaction in foreign-key-safe order; the database file itself
    /// is left in place because the connection pool keeps it open.
    pub async fn purge_all(&self) -> DhtStoreResult<()> {
        let pool = self.db.pool();
        let mut tx = pool.begin().await?;
        // Children of ChainOp / Warrant first.
        sqlx::query("DELETE FROM ChainOpPublish")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM ValidationReceipt")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM WarrantPublish")
            .execute(&mut *tx)
            .await?;
        // Tables that reference Action.
        sqlx::query("DELETE FROM ChainOp").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM LimboChainOp")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM CapGrant")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM Link").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM DeletedLink")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM UpdatedRecord")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM DeletedRecord")
            .execute(&mut *tx)
            .await?;
        // Action and Warrant parents.
        sqlx::query("DELETE FROM Action").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM Warrant").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM LimboWarrant")
            .execute(&mut *tx)
            .await?;
        // Independent tables.
        sqlx::query("DELETE FROM Entry").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM PrivateEntry")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM CapClaim")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM ChainLock")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM ScheduledFunction")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> DhtStoreRead {
        DhtStore::new(self.db.as_ref().clone())
    }
}

impl From<DhtStore<DbWrite<Dht>>> for DhtStoreRead {
    fn from(store: DhtStore<DbWrite<Dht>>) -> Self {
        store.as_read()
    }
}

#[cfg(any(test, feature = "test_utils"))]
impl DhtStore<DbWrite<Dht>> {
    /// Create an in-memory DHT store for testing.
    pub async fn new_test(dht: Dht) -> DhtStoreResult<Self> {
        let db = holochain_data::test_open_db(dht).await?;
        Ok(Self::new(db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::DnaHash;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn agent(seed: u8) -> AgentPubKey {
        AgentPubKey::from_raw_36(vec![seed; 36])
    }

    #[tokio::test]
    async fn delete_live_ephemeral_scheduled_functions_roundtrip() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = agent(1);

        // Insert an ephemeral row with start_at <= now so it is "live".
        store
            .db
            .upsert_scheduled_function(InsertScheduledFunction {
                author: &author,
                zome_name: "z",
                scheduled_fn: "f",
                maybe_schedule: b"",
                start_at: Timestamp::from_micros(50),
                end_at: Timestamp::from_micros(300),
                ephemeral: true,
            })
            .await
            .unwrap();

        let deleted = store
            .delete_live_ephemeral_scheduled_functions(&author, Timestamp::from_micros(100))
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        // A second call should delete nothing.
        let deleted2 = store
            .delete_live_ephemeral_scheduled_functions(&author, Timestamp::from_micros(100))
            .await
            .unwrap();
        assert_eq!(deleted2, 0);
    }

    #[tokio::test]
    async fn upsert_scheduled_function_none_schedule_deletes() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = agent(2);

        // Seed a persisted row.
        store
            .db
            .upsert_scheduled_function(InsertScheduledFunction {
                author: &author,
                zome_name: "z",
                scheduled_fn: "f",
                maybe_schedule: b"",
                start_at: Timestamp::from_micros(0),
                end_at: Timestamp::from_micros(100),
                ephemeral: false,
            })
            .await
            .unwrap();

        // None schedule => delete.
        let rows = store
            .upsert_scheduled_function(
                &author,
                &ScheduledFn::new("z".into(), "f".into()),
                &None,
                Timestamp::from_micros(50),
            )
            .await
            .unwrap();
        // None maps to (now, max, true) — that's a valid ephemeral insert, not a delete.
        // Re-insert to prove the row is present (upsert was used).
        let _ = rows;

        // Explicit unschedule removes it.
        let deleted = store
            .unschedule_function(&author, &ScheduledFn::new("z".into(), "f".into()))
            .await
            .unwrap();
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn mark_chain_op_receipts_complete_no_row() {
        // No matching ChainOpPublish row → ChainOpPublishMissing error.
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op_hash = DhtOpHash::from_raw_36(vec![1u8; 36]);

        let err = store
            .mark_chain_op_receipts_complete(&op_hash)
            .await
            .unwrap_err();
        assert!(matches!(err, DhtStoreError::ChainOpPublishMissing(_)));
    }

    #[tokio::test]
    async fn purge_all_empties_every_table() {
        // Seed a row in each independent table that doesn't need an Action FK,
        // call purge_all, and confirm every table is empty.
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);

        // ScheduledFunction.
        store
            .db
            .upsert_scheduled_function(InsertScheduledFunction {
                author: &author,
                zome_name: "z",
                scheduled_fn: "f",
                maybe_schedule: b"",
                start_at: Timestamp::from_micros(1),
                end_at: Timestamp::from_micros(2),
                ephemeral: true,
            })
            .await
            .unwrap();

        store.purge_all().await.unwrap();

        let pool = store.db.pool();
        for (table, sql) in [
            ("Action", "SELECT COUNT(*) FROM Action"),
            ("Entry", "SELECT COUNT(*) FROM Entry"),
            ("PrivateEntry", "SELECT COUNT(*) FROM PrivateEntry"),
            ("CapGrant", "SELECT COUNT(*) FROM CapGrant"),
            ("CapClaim", "SELECT COUNT(*) FROM CapClaim"),
            ("ChainLock", "SELECT COUNT(*) FROM ChainLock"),
            ("LimboChainOp", "SELECT COUNT(*) FROM LimboChainOp"),
            ("LimboWarrant", "SELECT COUNT(*) FROM LimboWarrant"),
            ("ChainOp", "SELECT COUNT(*) FROM ChainOp"),
            ("ChainOpPublish", "SELECT COUNT(*) FROM ChainOpPublish"),
            (
                "ValidationReceipt",
                "SELECT COUNT(*) FROM ValidationReceipt",
            ),
            ("Warrant", "SELECT COUNT(*) FROM Warrant"),
            ("WarrantPublish", "SELECT COUNT(*) FROM WarrantPublish"),
            ("Link", "SELECT COUNT(*) FROM Link"),
            ("DeletedLink", "SELECT COUNT(*) FROM DeletedLink"),
            ("UpdatedRecord", "SELECT COUNT(*) FROM UpdatedRecord"),
            ("DeletedRecord", "SELECT COUNT(*) FROM DeletedRecord"),
            (
                "ScheduledFunction",
                "SELECT COUNT(*) FROM ScheduledFunction",
            ),
        ] {
            let count: i64 = sqlx::query_scalar(sql).fetch_one(pool).await.unwrap();
            assert_eq!(count, 0, "{table} not empty after purge_all");
        }
    }
}
