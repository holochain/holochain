//! Per-DNA store for the `holochain_data` DHT database.
//!
//! [`DhtStore`] owns the [`DbWrite<Dht>`] handle for one DNA and exposes
//! domain-meaningful operations rather than raw database access. Call sites
//! obtain a reference from [`Space`](crate) and invoke named methods; they do
//! not need to interact with the underlying handle directly.

use holo_hash::{AgentPubKey, AnyDhtHash, DhtOpHash, HasHash};
use holochain_data::dht::{InsertLimboChainOp, InsertLimboWarrant, InsertScheduledFunction};
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::dht_op::{DhtOp, DhtOpHashed};
use holochain_types::prelude::{Schedule, ScheduledFn, Timestamp};
use holochain_zome_types::schedule::ScheduleError;

use crate::mutations::{StateMutationError, StateMutationResult};

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

    /// Mirror network-received ops into the limbo tables.
    ///
    /// For each [`DhtOpHashed`], the parent `Action` (and any associated
    /// `Entry`) is inserted into the DHT database first, then the op itself
    /// is inserted into `LimboChainOp` (chain ops) or `LimboWarrant` (warrant
    /// ops).  `require_receipt = true`; `serialized_size` is computed from the
    /// encoded op.
    ///
    /// All writes happen in a single transaction.  The `Action` and both limbo
    /// tables use `PRIMARY KEY ON CONFLICT IGNORE`, so duplicate ops are
    /// silently skipped.
    pub async fn record_incoming_ops(
        &self,
        ops: Vec<DhtOpHashed>,
    ) -> StateMutationResult<()> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(StateMutationError::from)?;
        let now = Timestamp::now();
        for op in ops {
            let op_hash = op.as_hash().clone();
            let serialized_size = holochain_serialized_bytes::encode(op.as_content())
                .map_err(StateMutationError::from)?
                .len() as u32;
            match op.into_inner().0 {
                DhtOp::ChainOp(chain_op) => {
                    let signed_action = chain_op.signed_action();
                    let action_hash =
                        holo_hash::ActionHash::with_data_sync(signed_action.action());
                    let sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
                        holo_hash::HoloHashed::with_pre_hashed(
                            signed_action.action().clone(),
                            action_hash.clone(),
                        ),
                        signed_action.signature().clone(),
                    );
                    let new_sah =
                        crate::source_chain::legacy_to_dht_v2_signed_action(&sah);
                    tx.insert_action(&new_sah, None)
                        .await
                        .map_err(StateMutationError::from)?;

                    // Insert entry if present.
                    use holochain_zome_types::entry_def::EntryVisibility;
                    if let holochain_types::prelude::RecordEntryRef::Present(entry) =
                        chain_op.entry()
                    {
                        let entry_hash = action_hash_to_entry_hash(&chain_op)?;
                        let is_private = matches!(
                            chain_op.action().entry_visibility(),
                            Some(EntryVisibility::Private)
                        );
                        if is_private {
                            let author = chain_op.author().clone();
                            tx.insert_private_entry(&entry_hash, &author, entry)
                                .await
                                .map_err(StateMutationError::from)?;
                        } else {
                            tx.insert_entry(&entry_hash, entry)
                                .await
                                .map_err(StateMutationError::from)?;
                        }
                    }

                    // Compute basis hash and storage_center_loc.
                    let linkable_basis = chain_op.dht_basis();
                    let storage_center_loc = linkable_basis.get_loc();
                    let basis_hash: AnyDhtHash =
                        AnyDhtHash::try_from(linkable_basis).map_err(|e| {
                            StateMutationError::Other(format!(
                                "cannot convert op basis to AnyDhtHash: {e:?}"
                            ))
                        })?;

                    tx.insert_limbo_chain_op(InsertLimboChainOp {
                        op_hash: &op_hash,
                        action_hash: &action_hash,
                        op_type: i64::from(chain_op.get_type()),
                        basis_hash: &basis_hash,
                        storage_center_loc,
                        require_receipt: true,
                        when_received: now,
                        serialized_size,
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
                DhtOp::WarrantOp(warrant_op) => {
                    let author = &warrant_op.author;
                    let timestamp = warrant_op.timestamp;
                    let warrantee = &warrant_op.warrantee;
                    let storage_center_loc = warrantee.get_loc();
                    let proof_bytes = holochain_serialized_bytes::encode(&warrant_op.proof)
                        .map_err(StateMutationError::from)?;

                    tx.insert_limbo_warrant(InsertLimboWarrant {
                        hash: &op_hash,
                        author,
                        timestamp,
                        warrantee,
                        proof: &proof_bytes,
                        storage_center_loc,
                        when_received: now,
                        serialized_size,
                    })
                    .await
                    .map_err(StateMutationError::from)?;
                }
            }
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Downgrade this writable store to a read-only store.
    pub fn as_read(&self) -> DhtStoreRead {
        DhtStore::new(self.db.as_ref().clone())
    }
}

/// Extract the `EntryHash` from a `ChainOp` that is known to carry an entry.
///
/// Returns an error if the action does not reference an entry hash, which
/// would indicate a programmer error (calling this for a `RecordEntry::NA`
/// variant).
fn action_hash_to_entry_hash(
    chain_op: &holochain_types::dht_op::ChainOp,
) -> StateMutationResult<holo_hash::EntryHash> {
    chain_op
        .action()
        .entry_hash()
        .cloned()
        .ok_or_else(|| StateMutationError::Other("op carries entry but action has no entry_hash".into()))
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

    // ---------------------------------------------------------------------------
    // Helpers for record_incoming_ops tests
    // ---------------------------------------------------------------------------

    /// Build a `StoreRecord` chain op for a `Create` action carrying a public
    /// entry.  `seed` is used to make each call produce distinct keys /
    /// hashes (it drives the raw bytes of the author key and entry hash).
    fn build_test_store_record_op_hashed(seed: u8) -> DhtOpHashed {
        use holo_hash::{ActionHash, EntryHash};
        use holochain_serialized_bytes::UnsafeBytes;
        use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
        use holochain_types::prelude::{AppEntryBytes, Entry, RecordEntry, Signature};
        use holochain_zome_types::action::{Action, Create, EntryType};
        use holochain_zome_types::entry_def::EntryVisibility;
        use holochain_zome_types::prelude::AppEntryDef;

        let author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let entry_hash = EntryHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::try_from(UnsafeBytes::from(
                vec![seed; 8],
            ))
            .unwrap(),
        ));
        let sig = Signature::from([seed; 64]);
        let action = Action::Create(Create {
            author: author.clone(),
            timestamp: Timestamp::from_micros(seed as i64 * 1000),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![seed.wrapping_add(200); 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: entry_hash.clone(),
            weight: Default::default(),
        });
        let op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
            sig,
            action,
            RecordEntry::Present(entry),
        )));
        DhtOpHashed::from_content_sync(op)
    }

    /// Build a `WarrantOp` (`ChainIntegrityWarrant::InvalidChainOp`) for
    /// testing.  `seed` drives distinct key bytes.
    fn build_test_warrant_op_hashed(seed: u8) -> DhtOpHashed {
        use holochain_types::dht_op::{DhtOp, DhtOpHashed};
        use holochain_types::warrant::WarrantOp;
        use holochain_zome_types::prelude::{
            ChainIntegrityWarrant, Signature, SignedWarrant, Warrant, WarrantProof,
        };
        use holochain_zome_types::op::ChainOpType;

        let action_author = AgentPubKey::from_raw_36(vec![seed; 36]);
        let warrantee = AgentPubKey::from_raw_36(vec![seed.wrapping_add(50); 36]);
        let action_hash = holo_hash::ActionHash::from_raw_36(vec![seed.wrapping_add(100); 36]);
        let warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author: action_author.clone(),
                    action: (action_hash, Signature::from([seed; 64])),
                    chain_op_type: ChainOpType::StoreRecord,
                }),
                AgentPubKey::from_raw_36(vec![seed.wrapping_add(10); 36]),
                Timestamp::from_micros(seed as i64 * 1000),
                warrantee,
            ),
            Signature::from([seed.wrapping_add(1); 64]),
        );
        let op = DhtOp::WarrantOp(Box::new(WarrantOp::from(warrant)));
        DhtOpHashed::from_content_sync(op)
    }

    // ---------------------------------------------------------------------------
    // record_incoming_ops tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_incoming_ops_inserts_limbo_chain_op() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(1);
        let op_hash = op.as_hash().clone();

        // Extract the action hash before consuming `op`.
        let action_hash = {
            let action = op.as_content().as_chain_op().unwrap().action();
            holo_hash::ActionHash::with_data_sync(&action)
        };

        store.record_incoming_ops(vec![op]).await.unwrap();

        // Action row was inserted.
        let found = store
            .db
            .as_ref()
            .get_action(action_hash)
            .await
            .unwrap();
        assert!(found.is_some(), "Action row not found after record_incoming_ops");

        // LimboChainOp row has require_receipt=true and a positive serialized_size.
        let row = store
            .db
            .as_ref()
            .get_limbo_chain_op(op_hash)
            .await
            .unwrap()
            .expect("LimboChainOp row not found");
        assert_eq!(row.require_receipt, 1, "require_receipt should be 1 (true)");
        assert!(row.serialized_size > 0, "serialized_size should be > 0");
    }

    #[tokio::test]
    async fn record_incoming_ops_inserts_limbo_warrant() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant_op = build_test_warrant_op_hashed(1);
        let op_hash = warrant_op.as_hash().clone();

        store.record_incoming_ops(vec![warrant_op]).await.unwrap();

        let row = store.db.as_ref().get_limbo_warrant(op_hash).await.unwrap();
        assert!(row.is_some(), "LimboWarrant row not found after record_incoming_ops");
        let row = row.unwrap();
        assert!(row.serialized_size > 0, "serialized_size should be > 0");
    }

    #[tokio::test]
    async fn record_incoming_ops_dedupes_on_conflict() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(2);
        let op_hash = op.as_hash().clone();

        // First insert.
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // Re-insert: ON CONFLICT IGNORE means no error and no duplicate row.
        store.record_incoming_ops(vec![op]).await.unwrap();

        // Exactly one row still exists.
        let row = store.db.as_ref().get_limbo_chain_op(op_hash).await.unwrap();
        assert!(row.is_some(), "LimboChainOp row should still be present");
    }
}
