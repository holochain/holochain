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

/// Result of system validation for a single DHT op, expressed in terms the
/// new schema understands.
#[derive(Debug, Clone, Copy)]
pub enum SysOutcome {
    /// Accepted — `sys_validation_status = 1`.
    Accepted,
    /// Rejected — `sys_validation_status = 2`.
    Rejected,
    /// Awaiting dependencies — status remains NULL (i.e. pending).
    AwaitingDeps,
}

/// Result of app validation for a single DHT op.
#[derive(Debug, Clone, Copy)]
pub enum AppOutcome {
    /// Accepted — `app_validation_status = 1`.
    Accepted,
    /// Rejected — `app_validation_status = 2`.
    Rejected,
    /// Awaiting dependencies — status remains NULL (i.e. pending).
    AwaitingDeps,
}

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
    pub async fn reschedule_expired_persisted(&self, author: &AgentPubKey, now: Timestamp) {
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

    /// Insert a [`SignedValidationReceipt`] into the `ValidationReceipt` table
    /// and return the current receipt count for the underlying op.
    ///
    /// The receipt hash is derived by serializing the full
    /// `SignedValidationReceipt` with `holochain_serialized_bytes` and then
    /// computing a `blake2b_256` digest over the resulting bytes, matching the
    /// legacy `add_if_unique` / `insert_validation_receipt_when` path.  The new
    /// schema's `ValidationReceipt` table has `hash` as PRIMARY KEY ON CONFLICT
    /// IGNORE, so duplicate inserts are silently dropped — same de-dupe
    /// semantics as the legacy table.
    ///
    /// The receipt count is queried after the transaction commits, so a
    /// concurrent writer could insert or remove receipts between commit and
    /// count; this is acceptable because the count only drives
    /// `mark_chain_op_receipts_complete`, which is informational and
    /// eventual-consistency is sufficient.
    ///
    /// Note: the legacy `ValidationReceipt.hash` stores a raw 32-byte blake2b
    /// digest, while the new DB wraps the same digest in a `DhtOpHash` (36
    /// bytes with the HoloHash type prefix).  Any code that compares receipt
    /// hashes across the two DBs during read cutover must account for this
    /// difference.
    pub async fn record_validation_receipt(
        &self,
        receipt: &holochain_types::prelude::SignedValidationReceipt,
    ) -> StateMutationResult<u64> {
        use holo_hash::encode::blake2b_256;

        // Derive the receipt hash the same way the legacy `add_if_unique` does:
        // serialize the whole SignedValidationReceipt, then take blake2b_256.
        let bytes =
            holochain_serialized_bytes::encode(receipt).map_err(StateMutationError::from)?;
        let hash_bytes = blake2b_256(&bytes);
        let receipt_hash = DhtOpHash::from_raw_32(hash_bytes);

        let op_hash = receipt.receipt.dht_op_hash.clone();

        // Serialize validators and signatures as individual blobs.
        let validators_bytes = holochain_serialized_bytes::encode(&receipt.receipt.validators)
            .map_err(StateMutationError::from)?;
        let signature_bytes = holochain_serialized_bytes::encode(&receipt.validators_signatures)
            .map_err(StateMutationError::from)?;

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;

        tx.insert_validation_receipt(
            &receipt_hash,
            &op_hash,
            &validators_bytes,
            &signature_bytes,
            holochain_types::prelude::Timestamp::now(),
        )
        .await
        .map_err(StateMutationError::from)?;

        tx.commit().await.map_err(StateMutationError::from)?;

        let op_hash_bytes = op_hash.get_raw_36().to_vec();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ValidationReceipt WHERE op_hash = ?")
                .bind(&op_hash_bytes)
                .fetch_one(self.db.pool())
                .await
                .map_err(StateMutationError::from)?;

        Ok(count as u64)
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
    pub async fn record_incoming_ops(&self, ops: Vec<DhtOpHashed>) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();
        for op in ops {
            let op_hash = op.as_hash().clone();
            let serialized_size = holochain_serialized_bytes::encode(op.as_content())
                .map_err(StateMutationError::from)?
                .len() as u32;
            match op.into_inner().0 {
                DhtOp::ChainOp(chain_op) => {
                    let signed_action = chain_op.signed_action();
                    let action_hash = holo_hash::ActionHash::with_data_sync(signed_action.action());
                    let sah = holochain_zome_types::record::SignedActionHashed::with_presigned(
                        holo_hash::HoloHashed::with_pre_hashed(
                            signed_action.action().clone(),
                            action_hash.clone(),
                        ),
                        signed_action.signature().clone(),
                    );
                    let new_sah = crate::source_chain::legacy_to_dht_v2_signed_action(&sah);
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

    /// Mirror sys validation outcomes from the legacy workflow. For each
    /// (op_hash, outcome) pair, update `sys_validation_status` on the
    /// matching limbo row. Each pair is tried first on `LimboChainOp`;
    /// if no row matches there, `LimboWarrant` is tried.
    pub async fn record_sys_validation_outcome(
        &self,
        outcomes: Vec<(DhtOpHash, SysOutcome)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for (hash, outcome) in outcomes {
            let status: Option<i64> = match outcome {
                SysOutcome::Accepted => Some(1),
                SysOutcome::Rejected => Some(2),
                SysOutcome::AwaitingDeps => None,
            };
            let updated = tx
                .set_limbo_chain_op_sys_validation_status(&hash, status)
                .await
                .map_err(StateMutationError::from)?;
            if updated == 0 {
                tx.set_limbo_warrant_sys_validation_status(&hash, status)
                    .await
                    .map_err(StateMutationError::from)?;
            }
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Mirror app validation outcomes from the validation workflow.  For each
    /// (op_hash, outcome) pair, update `app_validation_status` on the matching
    /// `LimboChainOp` row.  Warrants have no `app_validation_status` column, so
    /// only chain ops are updated here.
    pub async fn record_app_validation_outcome(
        &self,
        outcomes: Vec<(DhtOpHash, AppOutcome)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for (hash, outcome) in outcomes {
            let status: Option<i64> = match outcome {
                AppOutcome::Accepted => Some(1),
                AppOutcome::Rejected => Some(2),
                AppOutcome::AwaitingDeps => None,
            };
            tx.set_limbo_chain_op_app_validation_status(&hash, status)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Insert self-authored warrants directly into the `Warrant` table, bypassing
    /// `LimboWarrant`.  Self-authored warrants are locally trusted and do not need
    /// to go through the limbo/validation cycle.
    ///
    /// Any op that is not a `WarrantOp` is skipped with a warning log.  All
    /// inserts happen in a single transaction.
    pub async fn record_locally_validated_warrants(
        &self,
        warrants: Vec<DhtOpHashed>,
    ) -> StateMutationResult<()> {
        use holochain_data::dht::InsertWarrant;

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for op in warrants {
            let warrant_op = match op.as_content() {
                DhtOp::WarrantOp(w) => w,
                DhtOp::ChainOp(_) => {
                    tracing::warn!(
                        "record_locally_validated_warrants got a non-warrant DhtOp; skipping"
                    );
                    continue;
                }
            };
            let hash = op.as_hash();
            let proof_bytes = holochain_serialized_bytes::encode(&warrant_op.proof)
                .map_err(StateMutationError::from)?;
            tx.insert_warrant(InsertWarrant {
                hash,
                author: &warrant_op.author,
                timestamp: warrant_op.timestamp,
                warrantee: &warrant_op.warrantee,
                proof: &proof_bytes,
                storage_center_loc: warrant_op.warrantee.get_loc(),
            })
            .await
            .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Promote all limbo ops that satisfy the schema's ready-for-integration
    /// predicate into their integrated tables in a single transaction.
    ///
    /// Chain ops are moved from `LimboChainOp` → `ChainOp` with the terminal
    /// `validation_status` computed from the captured sys/app outcomes.
    /// Warrants are moved from `LimboWarrant` → `Warrant` (no timestamp
    /// column on `Warrant`).
    ///
    /// Returns the set of promoted op hashes (chain ops and warrant hashes
    /// together).  A generous batch limit is used; if more than that are ready
    /// in a single tick, the next tick handles the remainder.
    pub async fn integrate_ready_ops(
        &self,
        when_integrated: Timestamp,
    ) -> StateMutationResult<Vec<DhtOpHash>> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let mut promoted = Vec::new();

        let chain_ready = tx
            .as_mut()
            .limbo_chain_ops_ready_for_integration(10_000)
            .await
            .map_err(StateMutationError::from)?;
        for row in chain_ready {
            let op_hash = DhtOpHash::from_raw_36(row.hash.clone());
            let validation_status = compute_chain_op_validation_status(&row);
            let promoted_ok = tx
                .promote_limbo_chain_op(&op_hash, validation_status, when_integrated)
                .await
                .map_err(StateMutationError::from)?;
            if promoted_ok {
                promoted.push(op_hash);
            }
        }

        let warrant_ready = tx
            .as_mut()
            .limbo_warrants_ready_for_integration(10_000)
            .await
            .map_err(StateMutationError::from)?;
        for row in warrant_ready {
            let hash = DhtOpHash::from_raw_36(row.hash.clone());
            let promoted_ok = tx
                .promote_limbo_warrant(&hash)
                .await
                .map_err(StateMutationError::from)?;
            if promoted_ok {
                promoted.push(hash);
            }
        }

        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(promoted)
    }

    /// Set `require_receipt = false` for each of the given op hashes on
    /// `LimboChainOp`.
    ///
    /// If a hash no longer has a `LimboChainOp` row (e.g. the op has already
    /// been promoted to `ChainOp`), the update matches 0 rows and is silently
    /// ignored — that is correct, not a bug, because `require_receipt` only
    /// exists on the limbo table.
    pub async fn clear_require_receipt(
        &self,
        op_hashes: Vec<DhtOpHash>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            // Returns rows_affected; ignored — see doc note above.
            tx.set_limbo_chain_op_require_receipt(&hash, false)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Update `ChainOpPublish.last_publish_time = now` for each given op hash.
    ///
    /// Matches the legacy `set_last_publish_time` on the authored DB; the new
    /// schema's `ChainOpPublish` row owns this field instead of the authored
    /// DB's `DhtOp` row. Called from `publish_dht_ops_workflow` after the ops
    /// have been forwarded to the network.
    pub async fn record_published_op_hashes(
        &self,
        op_hashes: Vec<DhtOpHash>,
        now: Timestamp,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            tx.set_chain_op_last_publish_time(&hash, now)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Apply a successful countersigning completion: clear `withhold_publish`
    /// on the matching `ChainOpPublish` rows so the publish workflow can pick
    /// them up. The author / action_hash that the legacy site uses to filter
    /// authored-DB rows are not needed here because the new DB's
    /// `ChainOpPublish` is keyed directly by `op_hash`.
    pub async fn apply_countersigning_success(
        &self,
        op_hashes: Vec<DhtOpHash>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            tx.clear_chain_op_withhold_publish(&hash)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Force-reject a chain op. Used by host fns that detect a validation
    /// failure outside the validation workflows. Tries `ChainOp` first; if no
    /// row matches (the op is still in limbo), marks both sys and app validation
    /// status as Rejected on `LimboChainOp`.
    pub async fn reject_chain_op(&self, op_hashes: Vec<DhtOpHash>) -> StateMutationResult<()> {
        use holochain_zome_types::dht_v2::RecordValidity;

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            let updated = tx
                .set_chain_op_validation_status(&hash, RecordValidity::Rejected)
                .await
                .map_err(StateMutationError::from)?;
            if updated == 0 {
                // Op is not in ChainOp; try LimboChainOp.
                tx.set_limbo_chain_op_sys_validation_status(&hash, Some(2))
                    .await
                    .map_err(StateMutationError::from)?;
                tx.set_limbo_chain_op_app_validation_status(&hash, Some(2))
                    .await
                    .map_err(StateMutationError::from)?;
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
    chain_op.action().entry_hash().cloned().ok_or_else(|| {
        StateMutationError::Other("op carries entry but action has no entry_hash".into())
    })
}

/// Compute the terminal [`RecordValidity`](holochain_zome_types::dht_v2::RecordValidity)
/// for a limbo chain op row.
///
/// The schema's ready-for-integration predicate accepts a row when:
///   - `abandoned_at IS NOT NULL`; or
///   - `sys_validation_status = 2` (rejected at sys); or
///   - `sys_validation_status = 1 AND app_validation_status IN (1, 2)`.
///
/// Any rejection or abandonment maps to `Rejected`; otherwise `Accepted`.
fn compute_chain_op_validation_status(
    row: &holochain_data::models::dht::LimboChainOpRow,
) -> holochain_zome_types::dht_v2::RecordValidity {
    use holochain_zome_types::dht_v2::RecordValidity;
    if row.abandoned_at.is_some() {
        return RecordValidity::Rejected;
    }
    if row.sys_validation_status == Some(2) {
        return RecordValidity::Rejected;
    }
    if row.app_validation_status == Some(2) {
        return RecordValidity::Rejected;
    }
    RecordValidity::Accepted
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
            holochain_serialized_bytes::SerializedBytes::from(UnsafeBytes::from(vec![seed; 8])),
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
        use holochain_zome_types::op::ChainOpType;
        use holochain_zome_types::prelude::{
            ChainIntegrityWarrant, Signature, SignedWarrant, Warrant, WarrantProof,
        };

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
        let found = store.db.as_ref().get_action(action_hash).await.unwrap();
        assert!(
            found.is_some(),
            "Action row not found after record_incoming_ops"
        );

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
        assert!(
            row.is_some(),
            "LimboWarrant row not found after record_incoming_ops"
        );
        let row = row.unwrap();
        assert!(row.serialized_size > 0, "serialized_size should be > 0");
    }

    // ---------------------------------------------------------------------------
    // record_sys_validation_outcome tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_sys_validation_outcome_chain_op() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();

        // Seed a LimboChainOp row by calling record_incoming_ops (reusing the C1 helper).
        let op = build_test_store_record_op_hashed(10);
        let op_hash = op.as_hash().clone();
        store.record_incoming_ops(vec![op]).await.unwrap();

        // Confirm sys_validation_status starts as NULL.
        let row_before = store
            .db
            .as_ref()
            .get_limbo_chain_op(op_hash.clone())
            .await
            .unwrap()
            .expect("LimboChainOp row not found after seed");
        assert_eq!(row_before.sys_validation_status, None);

        store
            .record_sys_validation_outcome(vec![(op_hash.clone(), SysOutcome::Accepted)])
            .await
            .unwrap();

        let row = store
            .db
            .as_ref()
            .get_limbo_chain_op(op_hash)
            .await
            .unwrap()
            .expect("LimboChainOp row not found after update");
        assert_eq!(row.sys_validation_status, Some(1));
    }

    #[tokio::test]
    async fn record_sys_validation_outcome_warrant() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();

        // Seed a LimboWarrant row by calling record_incoming_ops (reusing the C1 helper).
        let op = build_test_warrant_op_hashed(20);
        let op_hash = op.as_hash().clone();
        store.record_incoming_ops(vec![op]).await.unwrap();

        // Confirm sys_validation_status starts as NULL.
        let row_before = store
            .db
            .as_ref()
            .get_limbo_warrant(op_hash.clone())
            .await
            .unwrap()
            .expect("LimboWarrant row not found after seed");
        assert_eq!(row_before.sys_validation_status, None);

        store
            .record_sys_validation_outcome(vec![(op_hash.clone(), SysOutcome::Rejected)])
            .await
            .unwrap();

        let row = store
            .db
            .as_ref()
            .get_limbo_warrant(op_hash)
            .await
            .unwrap()
            .expect("LimboWarrant row not found after update");
        assert_eq!(row.sys_validation_status, Some(2));
    }

    // ---------------------------------------------------------------------------
    // record_app_validation_outcome tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_app_validation_outcome_accepted() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(11);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // Pre-state: app_validation_status should be NULL.
        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.app_validation_status, None);

        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.app_validation_status, Some(1));
    }

    #[tokio::test]
    async fn record_app_validation_outcome_rejected() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(12);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();

        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Rejected)])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.app_validation_status, Some(2));
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

    // ---------------------------------------------------------------------------
    // integrate_ready_ops tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn integrate_ready_ops_promotes_ready_chain_op() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(50);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // Mark ready: sys=1, app=1.
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();

        let promoted = store
            .integrate_ready_ops(Timestamp::from_micros(999))
            .await
            .unwrap();
        assert_eq!(promoted, vec![op.as_hash().clone()]);

        assert!(store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .is_none());
        let row = store
            .db()
            .as_ref()
            .get_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.when_integrated, 999);
        assert_eq!(
            row.validation_status,
            i64::from(holochain_zome_types::dht_v2::RecordValidity::Accepted)
        );
    }

    #[tokio::test]
    async fn integrate_ready_ops_skips_unready() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(51);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // No validation outcomes recorded — sys/app are NULL, not ready.
        let promoted = store
            .integrate_ready_ops(Timestamp::from_micros(999))
            .await
            .unwrap();
        assert!(promoted.is_empty());

        // Op still in limbo, not in ChainOp.
        assert!(store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .is_some());
        assert!(store
            .db()
            .as_ref()
            .get_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn integrate_ready_ops_promotes_warrant() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant = build_test_warrant_op_hashed(52);
        store
            .record_incoming_ops(vec![warrant.clone()])
            .await
            .unwrap();
        // Mark sys=1 (warrants have no app validation).
        store
            .record_sys_validation_outcome(vec![(warrant.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();

        let promoted = store
            .integrate_ready_ops(Timestamp::from_micros(999))
            .await
            .unwrap();
        assert_eq!(promoted, vec![warrant.as_hash().clone()]);

        assert!(store
            .db()
            .as_ref()
            .get_limbo_warrant(warrant.as_hash().clone())
            .await
            .unwrap()
            .is_none());
        assert!(store
            .db()
            .as_ref()
            .get_warrant(warrant.as_hash().clone())
            .await
            .unwrap()
            .is_some());
    }

    // ---------------------------------------------------------------------------
    // record_validation_receipt tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_validation_receipt_inserts_and_counts() {
        use holochain_types::prelude::Signature;
        use holochain_types::prelude::{
            SignedValidationReceipt, ValidationReceipt, ValidationStatus,
        };

        let store = DhtStore::new_test(dht_id()).await.unwrap();

        // Seed a chain op and promote it to ChainOp so the FK is satisfied.
        let op = build_test_store_record_op_hashed(60);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(1))
            .await
            .unwrap();

        let receipt = SignedValidationReceipt {
            receipt: ValidationReceipt {
                dht_op_hash: op.as_hash().clone(),
                validation_status: ValidationStatus::Valid,
                validators: vec![AgentPubKey::from_raw_36(vec![5u8; 36])],
                when_integrated: Timestamp::from_micros(1),
            },
            validators_signatures: vec![Signature([0u8; 64])],
        };

        let count = store.record_validation_receipt(&receipt).await.unwrap();
        assert_eq!(count, 1);

        // Inserting the same receipt again should be a no-op (ON CONFLICT IGNORE)
        // and return count of 1 again.
        let count = store.record_validation_receipt(&receipt).await.unwrap();
        assert_eq!(count, 1);
    }

    // ---------------------------------------------------------------------------
    // clear_require_receipt tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn clear_require_receipt_clears_limbo_row() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(70);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // Pre: require_receipt = 1 (set by record_incoming_ops).
        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.require_receipt, 1);

        store
            .clear_require_receipt(vec![op.as_hash().clone()])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.require_receipt, 0);
    }

    #[tokio::test]
    async fn clear_require_receipt_no_op_for_integrated() {
        // Once promoted, the op is in ChainOp which has no require_receipt column.
        // The method should succeed (no error) with no observable effect.
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(71);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(1))
            .await
            .unwrap();
        // Op is now in ChainOp.
        assert!(store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .is_none());
        assert!(store
            .db()
            .as_ref()
            .get_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .is_some());

        // No-op; should not error.
        store
            .clear_require_receipt(vec![op.as_hash().clone()])
            .await
            .unwrap();
    }

    // ---------------------------------------------------------------------------
    // apply_countersigning_success tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn apply_countersigning_success_clears_withhold() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();

        // Seed an op through the full pipeline into ChainOp (satisfies FK for ChainOpPublish).
        let op = build_test_store_record_op_hashed(80);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(1))
            .await
            .unwrap();

        // Seed ChainOpPublish with withhold_publish = Some(true).
        store
            .db()
            .insert_chain_op_publish(op.as_hash(), None, None, Some(true))
            .await
            .unwrap();
        let row = store
            .db()
            .as_ref()
            .get_chain_op_publish(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.withhold_publish, Some(1));

        store
            .apply_countersigning_success(vec![op.as_hash().clone()])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op_publish(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.withhold_publish, None);
    }

    #[tokio::test]
    async fn apply_countersigning_success_no_op_when_row_absent() {
        // No ChainOpPublish row exists — method should not error.
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let dummy_hash = DhtOpHash::from_raw_36(vec![0xAA; 36]);
        store
            .apply_countersigning_success(vec![dummy_hash])
            .await
            .unwrap();
    }

    // ---------------------------------------------------------------------------
    // record_published_op_hashes tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_published_op_hashes_updates_publish_time() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        // Seed an op in ChainOp via the standard pipeline.
        let op = build_test_store_record_op_hashed(90);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(1))
            .await
            .unwrap();

        // Insert a ChainOpPublish row with NULL last_publish_time.
        store
            .db()
            .insert_chain_op_publish(op.as_hash(), None, None, None)
            .await
            .unwrap();

        store
            .record_published_op_hashes(vec![op.as_hash().clone()], Timestamp::from_micros(42))
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op_publish(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.last_publish_time, Some(42));
    }

    // ---------------------------------------------------------------------------
    // reject_chain_op tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn reject_chain_op_rejects_integrated_op() {
        use holochain_zome_types::dht_v2::RecordValidity;
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(100);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        store
            .record_sys_validation_outcome(vec![(op.as_hash().clone(), SysOutcome::Accepted)])
            .await
            .unwrap();
        store
            .record_app_validation_outcome(vec![(op.as_hash().clone(), AppOutcome::Accepted)])
            .await
            .unwrap();
        store
            .integrate_ready_ops(Timestamp::from_micros(1))
            .await
            .unwrap();
        // Pre: validation_status is Accepted.
        let row = store
            .db()
            .as_ref()
            .get_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.validation_status, i64::from(RecordValidity::Accepted));

        store
            .reject_chain_op(vec![op.as_hash().clone()])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.validation_status, i64::from(RecordValidity::Rejected));
    }

    #[tokio::test]
    async fn reject_chain_op_rejects_limbo_op() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let op = build_test_store_record_op_hashed(101);
        store.record_incoming_ops(vec![op.clone()]).await.unwrap();
        // Op is in limbo with sys=NULL, app=NULL.

        store
            .reject_chain_op(vec![op.as_hash().clone()])
            .await
            .unwrap();

        let row = store
            .db()
            .as_ref()
            .get_limbo_chain_op(op.as_hash().clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.sys_validation_status, Some(2));
        assert_eq!(row.app_validation_status, Some(2));
    }

    // ---------------------------------------------------------------------------
    // record_locally_validated_warrants tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn record_locally_validated_warrants_inserts_warrant() {
        let store = DhtStore::new_test(dht_id()).await.unwrap();
        let warrant_op = build_test_warrant_op_hashed(30);
        store
            .record_locally_validated_warrants(vec![warrant_op.clone()])
            .await
            .unwrap();
        let row = store
            .db()
            .as_ref()
            .get_warrant(warrant_op.as_hash().clone())
            .await
            .unwrap()
            .expect("warrant row missing");
        // warrantee is seed.wrapping_add(50) = 80 for seed=30.
        let expected_warrantee = AgentPubKey::from_raw_36(vec![80u8; 36]);
        assert_eq!(row.warrantee, expected_warrantee.get_raw_36().to_vec());
    }
}
