//! Per-DNA store for the `holochain_data` DHT database.
//!
//! [`DhtStore`] owns the [`DbWrite<Dht>`] handle for one DNA and exposes
//! domain-meaningful operations rather than raw database access. Call sites
//! obtain a reference from [`Space`](crate) and invoke named methods; they do
//! not need to interact with the underlying handle directly.

use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, DnaHash, EntryHash, HasHash};
use holochain_data::dht::{
    InsertLimboChainOp, InsertLimboWarrant, InsertScheduledFunction,
    RemoveCountersigningSessionOutcome,
};
use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::op::{DhtOp, DhtOpHashed};
use holochain_types::prelude::{Schedule, ScheduledFn, Timestamp};
use holochain_zome_types::schedule::ScheduleError;

use crate::mutations::{StateMutationError, StateMutationResult};
use crate::query::StateQueryResult;

/// Summary of a single op promoted by [`DhtStore::integrate_ready_ops`].
///
/// Captures the fields the integration workflow needs for metrics,
/// authored-op tracking, agent blocking, and `new_integrated_data`
/// notifications. The `basis_hash` field uses [`holo_hash::AnyLinkableHash`]
/// (the `OpBasis` alias) so that it covers Action, Entry, Agent, and link
/// base addresses — matching what `DhtOpHash::to_located_k2_op_id` expects.
#[derive(Debug, Clone)]
pub struct IntegratedOpSummary {
    /// Op hash (chain-op hash or warrant hash).
    pub op_hash: holo_hash::DhtOpHash,
    /// DHT basis hash (`OpBasis`) where the op is stored.
    /// `AnyLinkableHash`, not `AnyDhtHash`: link-op bases may be `External`
    /// hashes, which `AnyDhtHash` cannot hold.
    pub basis_hash: holo_hash::AnyLinkableHash,
    /// Authored timestamp of the underlying action or warrant.
    pub authored_timestamp: Timestamp,
    /// Terminal validation status for this op.
    pub validation_status: holochain_zome_types::action::OpValidity,
    /// When the op was received (used for the integration-delay metric).
    pub when_received: Timestamp,
    /// Combined validation attempts captured before promotion.
    pub validation_attempts: u32,
    /// Authoring agent for chain ops; `None` for warrants.
    pub action_author: Option<holo_hash::AgentPubKey>,
    /// Authoring agent for warrant ops; `None` for chain ops.
    pub warrant_author: Option<holo_hash::AgentPubKey>,
    /// Warrantee for warrant ops; `None` for chain ops.
    pub warrantee: Option<holo_hash::AgentPubKey>,
}

/// Output options for [`DhtStore::get_agent_activity`]. A `holochain_state`-local
/// mirror of `holochain_p2p`'s `GetActivityOptions` (which is off this crate's
/// dependency graph); the cascade maps between the two in phases 1b/1c.
#[derive(Debug, Clone, Copy)]
pub struct GetAgentActivityOptions {
    /// Include the valid activity in the response.
    pub include_valid_activity: bool,
    /// Include the rejected activity in the response.
    pub include_rejected_activity: bool,
    /// Include warrants in the response.
    pub include_warrants: bool,
    /// Return full records instead of just hashes.
    pub include_full_records: bool,
}

impl Default for GetAgentActivityOptions {
    fn default() -> Self {
        Self {
            include_valid_activity: true,
            include_rejected_activity: false,
            include_warrants: true,
            include_full_records: false,
        }
    }
}

/// Result of system validation for a single DHT op.
#[derive(Debug, Clone, Copy)]
pub enum SysOutcome {
    /// Accepted — `sys_validation_status = 1`.
    Accepted,
    /// Rejected — `sys_validation_status = 2`.
    Rejected,
}

/// Result of app validation for a single DHT op.
#[derive(Debug, Clone, Copy)]
pub enum AppOutcome {
    /// Accepted — `app_validation_status = 1`.
    Accepted,
    /// Rejected — `app_validation_status = 2`.
    Rejected,
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
    /// no matching `ChainOpPublish` row. Self-authored ops always have a
    /// `ChainOpPublish` row inserted during source-chain flush, so this
    /// indicates a wiring bug.
    #[error("no ChainOpPublish row for the given op_hash")]
    ChainOpPublishMissing,
}

/// Convenience alias for [`DhtStore`] results.
pub type DhtStoreResult<T> = Result<T, DhtStoreError>;

/// A read-only view of the DHT store.
pub type DhtStoreRead = DhtStore<holochain_data::DbRead<Dht>>;

// Re-exports of the row types returned by the K2 op-store reads, so
// downstream crates (`holochain_p2p`) can consume them without depending
// on `holochain_data` directly.
pub use holochain_data::models::dht::{
    K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow, K2WarrantForWireRow,
    SliceHashIndexedRow,
};

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
    /// The DNA hash this store belongs to.
    pub fn dna_hash(&self) -> &DnaHash {
        self.db.identifier().dna_hash()
    }

    /// Maximum number of connections in this store's underlying pool. Used by
    /// tests that assert the configured reader limit reaches the DB pool.
    #[cfg(any(test, feature = "inspection"))]
    pub fn connection_pool_max_size(&self) -> u32 {
        self.db.pool().options().get_max_connections()
    }

    /// Acquire the per-author source-chain write permit for this store.
    ///
    /// Serializes source-chain flushes for a single `(DNA, author)` chain so
    /// two concurrent flushes cannot both pass the as-at check and fork the
    /// chain. Different authors — and the same author on different DNAs — never
    /// block one another. See
    /// [`DbWrite::acquire_chain_write_permit`](holochain_data::DbWrite::acquire_chain_write_permit).
    pub async fn acquire_chain_write_permit(
        &self,
        author: &AgentPubKey,
    ) -> holochain_data::dht::ChainWritePermit {
        self.db.acquire_chain_write_permit(author).await
    }

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

    /// Delete every ephemeral scheduled-function row for this DNA, regardless
    /// of author or liveness. Returns the number of rows deleted.
    ///
    /// Called once per space at conductor startup to clear ephemeral schedules
    /// left over from a previous run — ephemeral schedules do not survive a
    /// reboot. A single call covers every author.
    pub async fn delete_all_ephemeral_scheduled_functions(&self) -> DhtStoreResult<u64> {
        Ok(self.db.delete_all_ephemeral_scheduled_functions().await?)
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

    /// Return the live scheduled functions for `author` at `now`.
    ///
    /// A function is "live" when `start_at <= now AND now <= end_at`.
    /// Returns `(ScheduledFn, Option<Schedule>, ephemeral)` tuples ordered by
    /// `start_at ASC`.
    pub async fn live_scheduled_functions(
        &self,
        author: &AgentPubKey,
        now: Timestamp,
    ) -> StateQueryResult<Vec<(ScheduledFn, Option<Schedule>, bool)>> {
        let rows = self
            .db
            .as_ref()
            .get_live_scheduled_functions(author, now)
            .await?;

        let mut result = Vec::with_capacity(rows.len());
        for (zome_name, fn_name, maybe_schedule_blob, ephemeral) in rows {
            let maybe_schedule: Option<Schedule> =
                holochain_serialized_bytes::decode(&maybe_schedule_blob)?;
            result.push((
                ScheduledFn::new(zome_name.into(), fn_name.into()),
                maybe_schedule,
                ephemeral,
            ));
        }
        Ok(result)
    }

    /// Insert a `SignedValidationReceipt` into the `ValidationReceipt` table
    /// and return the current receipt count for the underlying op.
    ///
    /// The receipt hash is derived by serializing the full
    /// `SignedValidationReceipt` with `holochain_serialized_bytes` and then
    /// computing a `blake2b_256` digest over the resulting bytes.  The
    /// `ValidationReceipt` table has `hash` as PRIMARY KEY ON CONFLICT
    /// IGNORE, so duplicate inserts are silently dropped.
    ///
    /// The receipt count is queried after the transaction commits, so a
    /// concurrent writer could insert or remove receipts between commit and
    /// count; this is acceptable because the count only drives
    /// `mark_chain_op_receipts_complete`, which is informational and
    /// eventual-consistency is sufficient.
    pub async fn record_validation_receipt(
        &self,
        receipt: &holochain_types::prelude::SignedValidationReceipt,
    ) -> StateMutationResult<u64> {
        use holo_hash::encode::blake2b_256;

        // Derive the receipt hash: serialize the whole SignedValidationReceipt,
        // then take blake2b_256.
        let bytes =
            holochain_serialized_bytes::encode(receipt).map_err(StateMutationError::from)?;
        let hash_bytes = blake2b_256(&bytes);
        let receipt_hash = DhtOpHash::from_raw_32(hash_bytes);

        let op_hash = receipt.receipt.dht_op_hash.clone();

        // Store the full serialized `SignedValidationReceipt` (`bytes` above),
        // so readers can reconstruct the validator-reported status + validators.
        self.db
            .insert_validation_receipt(
                &receipt_hash,
                &op_hash,
                &bytes,
                holochain_types::prelude::Timestamp::now(),
            )
            .await
            .map_err(StateMutationError::from)?;

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
    /// which indicates that no `ChainOpPublish` row was inserted for this
    /// self-authored op.
    pub async fn mark_chain_op_receipts_complete(&self, op_hash: &DhtOpHash) -> DhtStoreResult<()> {
        let rows = self.db.set_chain_op_receipts_complete(op_hash).await?;
        if rows == 0 {
            return Err(DhtStoreError::ChainOpPublishMissing);
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
        // Action and Warrant parents (warrant op metadata first, since both
        // LimboWarrantOp and WarrantOp reference Warrant via FK).
        sqlx::query("DELETE FROM Action").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM LimboWarrantOp")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM WarrantOp")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM Warrant").execute(&mut *tx).await?;
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
        sqlx::query("DELETE FROM SliceHash")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Store network-received ops in the limbo tables for validation.
    ///
    /// For each [`DhtOpHashed`], the parent `Action` (and any associated
    /// `Entry`) is inserted into the DHT database first, then the op itself
    /// is inserted into `LimboChainOp` (chain ops) or `Warrant` +
    /// `LimboWarrantOp` (warrant ops).  `require_receipt = true`;
    /// `serialized_size` is provided by the
    /// caller and should reflect the size of the op as received from the network.
    ///
    /// The bool indicates if a validation receipt is required for the op.
    ///
    /// All writes happen in a single transaction.  The `Action` and both limbo
    /// tables use `PRIMARY KEY ON CONFLICT IGNORE`, so duplicates are
    /// silently skipped.
    pub async fn record_incoming_ops(
        &self,
        ops: Vec<(DhtOpHashed, bool)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();
        for (op, require_receipt) in ops {
            let op_hash = op.as_hash().clone();
            let serialized_size = holochain_serialized_bytes::encode(op.as_content())
                .map_err(StateMutationError::from)?
                .len() as u32;
            match op.into_inner().0 {
                DhtOp::ChainOp(chain_op) => {
                    let signed_action = chain_op.signed_action();
                    let action_hash = holo_hash::ActionHash::with_data_sync(signed_action.data());
                    let sah = holochain_zome_types::action::SignedActionHashed::with_presigned(
                        holo_hash::HoloHashed::with_pre_hashed(
                            signed_action.data().clone(),
                            action_hash.clone(),
                        ),
                        signed_action.signature().clone(),
                    );
                    tx.insert_action(&sah, None)
                        .await
                        .map_err(StateMutationError::from)?;

                    // Insert entry if present.
                    // Network-received ops should never carry private entries.
                    if let Some(holochain_types::op::OpEntry::Present(entry)) = chain_op.op_entry()
                    {
                        let entry_hash = entry_hash_from_chain_op_action(signed_action.data())?;
                        tx.insert_entry(&entry_hash, entry)
                            .await
                            .map_err(StateMutationError::from)?;
                    }

                    // Compute basis hash and storage_center_loc.
                    let basis_hash = chain_op.dht_basis();
                    let storage_center_loc = basis_hash.get_loc();

                    tx.insert_limbo_chain_op(InsertLimboChainOp {
                        op_hash: &op_hash,
                        action_hash: &action_hash,
                        op_type: i64::from(chain_op.op_type()),
                        basis_hash: &basis_hash,
                        storage_center_loc,
                        require_receipt,
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
                    let signature_bytes = warrant_op.signature().0;

                    tx.insert_limbo_warrant(InsertLimboWarrant {
                        hash: &op_hash,
                        author,
                        timestamp,
                        warrantee,
                        proof: &proof_bytes,
                        signature: &signature_bytes,
                        reason: warrant_op.proof.reason(),
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

    /// Record the system validation outcome for each chain op.
    ///
    /// For each (op_hash, outcome) pair, updates `sys_validation_status` on the
    /// matching `LimboChainOp` row.
    pub async fn record_chain_op_sys_validation_outcomes(
        &self,
        outcomes: Vec<(DhtOpHash, SysOutcome)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for (hash, outcome) in outcomes {
            let status: i64 = match outcome {
                SysOutcome::Accepted => 1,
                SysOutcome::Rejected => 2,
            };
            tx.set_limbo_chain_op_sys_validation_status(&hash, Some(status))
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Record the system validation outcome for each warrant op.
    ///
    /// For each (op_hash, outcome) pair, updates `sys_validation_status` on the
    /// matching `LimboWarrantOp` row.
    pub async fn record_warrant_sys_validation_outcomes(
        &self,
        outcomes: Vec<(DhtOpHash, SysOutcome)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for (hash, outcome) in outcomes {
            let status: i64 = match outcome {
                SysOutcome::Accepted => 1,
                SysOutcome::Rejected => 2,
            };
            tx.set_limbo_warrant_sys_validation_status(&hash, Some(status))
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Record the app validation outcome for each op.  For each
    /// (op_hash, outcome) pair, update `app_validation_status` on the matching
    /// `LimboChainOp` row.  Warrants have no `app_validation_status` column, so
    /// only chain ops are updated here.
    pub async fn record_app_validation_outcomes(
        &self,
        outcomes: Vec<(DhtOpHash, AppOutcome)>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for (hash, outcome) in outcomes {
            let status: i64 = match outcome {
                AppOutcome::Accepted => 1,
                AppOutcome::Rejected => 2,
            };
            tx.set_limbo_chain_op_app_validation_status(&hash, Some(status))
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Insert self-authored warrants into limbo (`Warrant` + `LimboWarrantOp`)
    /// already marked sys-validation accepted, so they are immediately ready
    /// for integration.
    ///
    /// Self-authored warrants are locally trusted, but they must still pass
    /// through the integration workflow: that is where the warrantee is blocked
    /// (via [`integrate_ready_ops`](Self::integrate_ready_ops)'s
    /// [`IntegratedOpSummary`]). Inserting them straight into `WarrantOp` would
    /// bypass that path and the block would never fire.
    ///
    /// Any op that is not a `WarrantOp` is skipped with a warning log.  All
    /// inserts happen in a single transaction.
    pub async fn record_locally_validated_warrants(
        &self,
        warrants: Vec<DhtOpHashed>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let now = Timestamp::now();
        for op in warrants {
            let serialized_size = holochain_serialized_bytes::encode(op.as_content())
                .map_err(StateMutationError::from)?
                .len() as u32;
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
            let signature_bytes = warrant_op.signature().0;
            tx.insert_limbo_warrant(InsertLimboWarrant {
                hash,
                author: &warrant_op.author,
                timestamp: warrant_op.timestamp,
                warrantee: &warrant_op.warrantee,
                proof: &proof_bytes,
                signature: &signature_bytes,
                reason: warrant_op.proof.reason(),
                storage_center_loc: warrant_op.warrantee.get_loc(),
                when_received: now,
                serialized_size,
            })
            .await
            .map_err(StateMutationError::from)?;
            // Mark accepted so `integrate_ready_ops` promotes it on the next
            // integration tick (warrants have no app-validation stage).
            tx.set_limbo_warrant_sys_validation_status(hash, Some(1))
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
    /// Warrants are promoted by moving their op metadata from `LimboWarrantOp`
    /// → `WarrantOp`; the shared `Warrant` content row stays put.
    ///
    /// Returns per-op summary data for each promoted op (chain ops and warrants
    /// together). The summary includes the basis hash, authored timestamp,
    /// validation status, received time, validation attempt counts, and
    /// author/warrantee fields needed by the integration workflow for metrics,
    /// agent blocking, and `new_integrated_data` notifications.
    ///
    /// A generous batch limit is used; if more than that are ready in a single
    /// tick, the next tick handles the remainder.
    pub async fn integrate_ready_ops(
        &self,
        when_integrated: Timestamp,
    ) -> StateMutationResult<Vec<IntegratedOpSummary>> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let mut out: Vec<IntegratedOpSummary> = Vec::new();

        let chain_ready = tx
            .as_mut()
            .limbo_chain_ops_ready_for_integration(10_000)
            .await
            .map_err(StateMutationError::from)?;
        for row in chain_ready {
            let op_hash = DhtOpHash::from_raw_36(row.hash.clone());
            let validation_status = compute_chain_op_validation_status(&row);

            // Reconstruct the basis hash from op_type + raw bytes.
            // The new-schema DB stores only 36 raw bytes (no type prefix), so
            // we recover the type from the op_type discriminant.
            let basis_hash = chain_op_basis_hash_from_row(row.op_type, row.basis_hash.clone());

            // Look up the action to recover author + authored timestamp.
            let action_hash = holo_hash::ActionHash::from_raw_36(row.action_hash.clone());
            let action = tx
                .as_mut()
                .get_action(action_hash.clone())
                .await
                .map_err(StateMutationError::from)?;
            let (action_author, authored_timestamp, action_data) = match action {
                Some(sah) => (
                    Some(sah.hashed.content.header.author.clone()),
                    sah.hashed.content.header.timestamp,
                    Some(sah.hashed.content.data.clone()),
                ),
                None => (None, Timestamp::from_micros(0), None),
            };

            let promoted_ok = tx
                .promote_limbo_chain_op(&op_hash, validation_status, when_integrated)
                .await
                .map_err(StateMutationError::from)?;
            if promoted_ok {
                // Populate the per-action index tables for the integrated action,
                // mirroring `cache_chain_ops`, so integrated incoming data
                // (links, deletes, updates) is queryable via the indexes.
                if let Some(action_data) = action_data {
                    action_indexes::insert_action_indexes(&mut tx, &action_hash, &action_data)
                        .await?;
                }

                out.push(IntegratedOpSummary {
                    op_hash,
                    basis_hash,
                    authored_timestamp,
                    validation_status,
                    when_received: Timestamp::from_micros(row.when_received),
                    validation_attempts: (row.sys_validation_attempts + row.app_validation_attempts)
                        as u32,
                    action_author,
                    warrant_author: None,
                    warrantee: None,
                });
            }
        }

        let warrant_ready = tx
            .as_mut()
            .limbo_warrants_ready_for_integration(10_000)
            .await
            .map_err(StateMutationError::from)?;
        for row in warrant_ready {
            let op_hash = DhtOpHash::from_raw_36(row.hash.clone());
            let promoted_ok = tx
                .promote_limbo_warrant(&op_hash, when_integrated)
                .await
                .map_err(StateMutationError::from)?;
            if promoted_ok {
                let validation_status = match row.sys_validation_status {
                    Some(2) => holochain_zome_types::action::OpValidity::Rejected,
                    _ => holochain_zome_types::action::OpValidity::Accepted,
                };
                // Warrant basis = warrantee (AgentPubKey hash).
                let warrantee = holo_hash::AgentPubKey::from_raw_36(row.warrantee.clone());
                let basis_hash: holo_hash::AnyLinkableHash = warrantee.clone().into();
                out.push(IntegratedOpSummary {
                    op_hash,
                    basis_hash,
                    authored_timestamp: Timestamp::from_micros(row.timestamp),
                    validation_status,
                    when_received: Timestamp::from_micros(row.when_received),
                    validation_attempts: row.sys_validation_attempts as u32,
                    action_author: None,
                    warrant_author: Some(holo_hash::AgentPubKey::from_raw_36(row.author.clone())),
                    warrantee: Some(warrantee),
                });
            }
        }

        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(out)
    }

    /// Clear `require_receipt = 0` on the `ChainOp` row for each given op hash.
    /// Called by the validation receipt workflow after a receipt has been sent.
    pub async fn clear_require_receipts(
        &self,
        op_hashes: Vec<DhtOpHash>,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            tx.clear_chain_op_require_receipt(&hash)
                .await
                .map_err(StateMutationError::from)?;
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Record that each given op hash has just been published, updating its
    /// publish-state timestamp.
    ///
    /// Chain ops have their `ChainOpPublish.last_publish_time` set to `now`.
    /// A hash with no matching `ChainOpPublish` row is a warrant: warrants
    /// publish at most once, so a `WarrantPublish` row recording `now` is
    /// inserted instead (the `PRIMARY KEY ON CONFLICT IGNORE` keeps the first
    /// publish time if the same warrant is recorded again). The publish queue
    /// excludes any warrant that has a `WarrantPublish` row.
    pub async fn record_published_op_hashes(
        &self,
        op_hashes: Vec<DhtOpHash>,
        now: Timestamp,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            let updated = tx
                .set_chain_op_last_publish_time(&hash, now)
                .await
                .map_err(StateMutationError::from)?;
            if updated == 0 {
                tx.insert_warrant_publish(&hash, Some(now))
                    .await
                    .map_err(StateMutationError::from)?;
            }
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Clear `withhold_publish` on the `ChainOpPublish` rows for the given
    /// op hashes so the publish workflow can pick them up.
    pub async fn clear_op_withhold_publishes(
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
    pub async fn reject_chain_ops(&self, op_hashes: Vec<DhtOpHash>) -> StateMutationResult<()> {
        use holochain_zome_types::action::OpValidity;

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        for hash in op_hashes {
            let updated = tx
                .set_chain_op_validation_status(&hash, OpValidity::Rejected)
                .await
                .map_err(StateMutationError::from)?;
            if updated == 0 {
                // Op is not in ChainOp; try LimboChainOp, force-rejecting
                // regardless of current validation state.
                tx.force_reject_limbo_chain_op(&hash)
                    .await
                    .map_err(StateMutationError::from)?;
            }
        }
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Re-queue a cache-derived op for validation.
    ///
    /// If a `ChainOp` row matching `(action_hash, op_type)` with
    /// `locally_validated = false` exists, move it back into `LimboChainOp`
    /// with cleared validation status so the next sys-validation pass picks it
    /// up via `ops_pending_sys_validation`.
    ///
    /// Returns `Ok(true)` if a row was moved, `Ok(false)` if no matching
    /// cached row exists (e.g. the op was never cached, or was already locally
    /// validated).
    pub async fn move_warranted_op_to_limbo(
        &self,
        action_hash: &holo_hash::ActionHash,
        op_type: holochain_zome_types::op::ChainOpType,
    ) -> StateMutationResult<bool> {
        Ok(self
            .db
            .move_chain_op_to_limbo(action_hash, i64::from(op_type))
            .await?)
    }

    /// Try to acquire the source-chain lock for `author`.
    ///
    /// Returns `Ok(true)` when the caller holds the lock (no lock existed,
    /// the existing lock had expired relative to `now`, or the existing lock's
    /// `subject` matched and was therefore extended), and `Ok(false)` when a
    /// different subject still holds an unexpired lock.
    pub async fn acquire_chain_lock(
        &self,
        author: &AgentPubKey,
        subject: &[u8],
        expires_at: Timestamp,
        now: Timestamp,
    ) -> StateMutationResult<bool> {
        Ok(self
            .db
            .acquire_chain_lock(author, subject, expires_at, now)
            .await?)
    }

    /// Release the source-chain lock for `author` by deleting the lock row.
    ///
    /// Releasing a non-existent lock is a no-op.
    pub async fn release_chain_lock(&self, author: &AgentPubKey) -> StateMutationResult<()> {
        self.db.release_chain_lock(author).await?;
        Ok(())
    }

    /// Force-remove a self-authored countersigning session (its `Action`,
    /// `ChainOp`/`ChainOpPublish` rows and entry) from the DhtStore,
    /// identified by `(action_hash, entry_hash)`.
    ///
    /// This is defensive about sessions whose ops have already been published:
    /// if any of the action's ops has a `ChainOpPublish` row with
    /// `withhold_publish IS NULL` the removal is refused with
    /// [`StateMutationError::CannotRemoveFullyPublished`] and no rows are
    /// touched. The guard and deletes run in a single transaction.
    pub async fn remove_countersigning_session(
        &self,
        action_hash: ActionHash,
        entry_hash: EntryHash,
    ) -> StateMutationResult<()> {
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        let outcome = tx
            .remove_countersigning_session(&action_hash, &entry_hash)
            .await
            .map_err(StateMutationError::from)?;
        match outcome {
            RemoveCountersigningSessionOutcome::AlreadyPublished => {
                // Drop the transaction without committing (no rows were
                // written) and refuse the removal.
                Err(StateMutationError::CannotRemoveFullyPublished)
            }
            RemoveCountersigningSessionOutcome::Removed => {
                tx.commit().await.map_err(StateMutationError::from)?;
                Ok(())
            }
        }
    }
}

impl<Db> DhtStore<Db>
where
    Db: AsRef<holochain_data::DbRead<Dht>>,
{
    /// Downgrade to a read-only store over the same database handle.
    ///
    /// Works for both a writable store (`DbWrite<Dht>`) and a store that is
    /// already read-only (`DbRead<Dht>`), since both handle types provide
    /// `AsRef<DbRead<Dht>>`.
    pub fn as_read(&self) -> DhtStoreRead {
        DhtStore::new(self.db.as_ref().clone())
    }
}

/// Extract the `EntryHash` from an `Action` that is known to carry an entry.
///
/// Returns an error if the action does not reference an entry hash, which
/// would indicate a programmer error (calling this for an `OpEntry::ActionOnly`
/// variant).
fn entry_hash_from_chain_op_action(
    action: &holochain_zome_types::action::Action,
) -> StateMutationResult<holo_hash::EntryHash> {
    use holochain_zome_types::action::ActionData;
    match &action.data {
        ActionData::Create(d) => Ok(d.entry_hash.clone()),
        ActionData::Update(d) => Ok(d.entry_hash.clone()),
        _ => Err(StateMutationError::Other(
            "op carries entry but action has no entry_hash".into(),
        )),
    }
}

/// Reconstruct a DHT basis hash from a `LimboChainOp` row.
///
/// The new schema stores `basis_hash` as raw 36 bytes (no type prefix), so
/// the type must be inferred from `op_type`. The mapping follows
/// `docs/design/state_model.md` and `holochain_zome_types::op::ChainOpType`:
///
/// | `op_type` | Basis hash type |
/// |-----------|-----------------|
/// | 1 (StoreRecord)                 | `ActionHash`  |
/// | 2 (StoreEntry)                  | `EntryHash`   |
/// | 3 (RegisterAgentActivity)       | `AgentPubKey` |
/// | 4 (RegisterUpdatedContent)      | `EntryHash`   |
/// | 5 (RegisterUpdatedRecord)       | `ActionHash`  |
/// | 6 (RegisterDeletedEntryAction)  | `EntryHash`   |
/// | 7 (RegisterDeletedBy)           | `ActionHash`  |
/// | 8 (RegisterAddLink)             | `EntryHash`   |
/// | 9 (RegisterRemoveLink)          | `EntryHash`   |
///
/// Link bases (8, 9) can technically be any `AnyLinkableHash` variant, but
/// the new schema stores them in the same 36-byte slot. Non-Holochain external
/// hashes are not representable in this schema and would not reach integration;
/// `EntryHash` is used as the fallback for those rows.
fn chain_op_basis_hash_from_row(op_type: i64, raw: Vec<u8>) -> holo_hash::AnyLinkableHash {
    match op_type {
        // StoreRecord, RegisterUpdatedRecord, RegisterDeletedBy → ActionHash basis
        1 | 5 | 7 => holo_hash::ActionHash::from_raw_36(raw).into(),
        // RegisterAgentActivity → AgentPubKey basis
        3 => holo_hash::AgentPubKey::from_raw_36(raw).into(),
        // StoreEntry, RegisterUpdatedContent, RegisterDeletedEntryAction,
        // RegisterAddLink, RegisterRemoveLink → EntryHash basis (or Agent as Entry)
        _ => holo_hash::EntryHash::from_raw_36(raw).into(),
    }
}

/// Compute the terminal [`OpValidity`](holochain_zome_types::action::OpValidity)
/// for a limbo chain op row.
///
/// The schema's ready-for-integration predicate accepts a row when:
///   - `sys_validation_status = 2` (rejected at sys); or
///   - `sys_validation_status = 1 AND app_validation_status IN (1, 2)`.
///
/// Any rejection maps to `Rejected`; otherwise `Accepted`.
fn compute_chain_op_validation_status(
    row: &holochain_data::models::dht::LimboChainOpRow,
) -> holochain_zome_types::action::OpValidity {
    use holochain_zome_types::action::OpValidity as RecordValidity;
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

#[cfg(feature = "test_utils")]
impl DhtStore<DbWrite<Dht>> {
    /// Create an in-memory DHT store for testing.
    pub async fn new_test(dht: Dht) -> DhtStoreResult<Self> {
        let db = holochain_data::test_open_db(dht).await?;
        Ok(Self::new(db))
    }

    /// Return the `when_integrated` timestamp for the given op hash if the op
    /// is present in the `ChainOp` table (i.e. it has been promoted from limbo
    /// and fully integrated). Returns `None` when the op is not yet integrated.
    pub async fn when_integrated(
        &self,
        op_hash: &holo_hash::DhtOpHash,
    ) -> DhtStoreResult<Option<Timestamp>> {
        let row = self.db.as_ref().get_chain_op(op_hash.clone()).await?;
        Ok(row.map(|r| Timestamp::from_micros(r.when_integrated)))
    }

    /// Test-only helper that writes a warrant op straight into the integrated
    /// `Warrant` + `WarrantOp` tables (with `when_integrated = now`), bypassing
    /// `LimboWarrantOp` and the integration workflow's block trigger.
    ///
    /// Use this to seed a warrant for tests that need it visible to K2 gossip
    /// without invoking the integration workflow's `block_agents` path — i.e.
    /// to inject a warrant that should reach a peer for the peer to evaluate,
    /// without the author also blocking the warrantee locally.
    pub async fn test_insert_integrated_warrant(
        &self,
        warrant: DhtOpHashed,
    ) -> StateMutationResult<()> {
        use holochain_data::dht::InsertWarrant;

        let warrant_op = match warrant.as_content() {
            DhtOp::WarrantOp(w) => w,
            DhtOp::ChainOp(_) => panic!("test_insert_integrated_warrant requires a WarrantOp"),
        };
        let serialized_size = holochain_serialized_bytes::encode(warrant.as_content())
            .map_err(StateMutationError::from)?
            .len() as u32;
        let proof_bytes = holochain_serialized_bytes::encode(&warrant_op.proof)
            .map_err(StateMutationError::from)?;
        let signature_bytes = warrant_op.signature().0;
        let now = Timestamp::now();
        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        tx.insert_warrant(InsertWarrant {
            hash: warrant.as_hash(),
            author: &warrant_op.author,
            timestamp: warrant_op.timestamp,
            warrantee: &warrant_op.warrantee,
            proof: &proof_bytes,
            signature: &signature_bytes,
            reason: warrant_op.proof.reason(),
            storage_center_loc: warrant_op.warrantee.get_loc(),
            when_received: now,
            when_integrated: now,
            validation_status: 1,
            serialized_size,
        })
        .await
        .map_err(StateMutationError::from)?;
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Test-only: insert a self-authored chain op as fully integrated
    /// (`locally_validated = true`, `when_integrated = now`) together with a
    /// `ChainOpPublish` row carrying the given publish state.
    ///
    /// The parent `Action` is inserted first (its `private_entry` flag is
    /// derived from the action's entry visibility). Entries are not stored,
    /// since the publish queue does not read them.
    pub async fn test_insert_authored_chain_op(
        &self,
        op: DhtOpHashed,
        last_publish_time: Option<Timestamp>,
        receipts_complete: Option<bool>,
        withhold_publish: Option<bool>,
    ) -> StateMutationResult<()> {
        use holochain_data::dht::InsertChainOp;
        use holochain_zome_types::action::RecordValidity;

        let op_hash = op.as_hash().clone();
        let serialized_size = holochain_serialized_bytes::encode(op.as_content())
            .map_err(StateMutationError::from)?
            .len() as u32;
        let chain_op = match op.into_inner().0 {
            DhtOp::ChainOp(c) => c,
            DhtOp::WarrantOp(_) => panic!("test_insert_authored_chain_op requires a ChainOp"),
        };

        let signed_action = chain_op.signed_action();
        let action_hash = holo_hash::ActionHash::with_data_sync(signed_action.data());
        let sah = holochain_zome_types::action::SignedActionHashed::with_presigned(
            holo_hash::HoloHashed::with_pre_hashed(
                signed_action.data().clone(),
                action_hash.clone(),
            ),
            signed_action.signature().clone(),
        );

        let basis_hash = chain_op.dht_basis();
        let storage_center_loc = basis_hash.get_loc();
        let now = Timestamp::now();

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        tx.insert_action(&sah, Some(RecordValidity::Accepted))
            .await
            .map_err(StateMutationError::from)?;
        tx.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: i64::from(chain_op.op_type()),
            basis_hash: &basis_hash,
            storage_center_loc,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: true,
            when_received: now,
            when_integrated: now,
            serialized_size,
        })
        .await
        .map_err(StateMutationError::from)?;
        tx.insert_chain_op_publish(
            &op_hash,
            last_publish_time,
            receipts_complete,
            withhold_publish,
        )
        .await
        .map_err(StateMutationError::from)?;
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Test-only: insert an integrated self-authored chain op
    /// (`locally_validated = true`, `when_integrated = now`) plus its
    /// `ChainOpPublish` row, WITHOUT inserting the parent `Action`.
    ///
    /// A committed record produces several ops that share one action — a
    /// `Create`, for instance, yields `StoreRecord`, `RegisterAgentActivity`,
    /// and `StoreEntry` ops — all written integrated by the source-chain flush.
    /// Use this after
    /// [`test_insert_authored_chain_op`](DhtStore::test_insert_authored_chain_op),
    /// which inserts the action once, to add the sibling op types for the same
    /// action without colliding on the `Action` primary key.
    pub async fn test_insert_additional_integrated_op(
        &self,
        op: DhtOpHashed,
        withhold_publish: Option<bool>,
    ) -> StateMutationResult<()> {
        use holochain_data::dht::InsertChainOp;
        use holochain_zome_types::action::RecordValidity;

        let op_hash = op.as_hash().clone();
        let serialized_size = holochain_serialized_bytes::encode(op.as_content())
            .map_err(StateMutationError::from)?
            .len() as u32;
        let chain_op = match op.into_inner().0 {
            DhtOp::ChainOp(c) => c,
            DhtOp::WarrantOp(_) => {
                panic!("test_insert_additional_integrated_op requires a ChainOp")
            }
        };

        let action_hash = holo_hash::ActionHash::with_data_sync(chain_op.signed_action().data());
        let basis_hash = chain_op.dht_basis();
        let storage_center_loc = basis_hash.get_loc();
        let now = Timestamp::now();

        let mut tx = self.db.begin().await.map_err(StateMutationError::from)?;
        tx.insert_chain_op(InsertChainOp {
            op_hash: &op_hash,
            action_hash: &action_hash,
            op_type: i64::from(chain_op.op_type()),
            basis_hash: &basis_hash,
            storage_center_loc,
            validation_status: RecordValidity::Accepted,
            locally_validated: true,
            require_receipt: true,
            when_received: now,
            when_integrated: now,
            serialized_size,
        })
        .await
        .map_err(StateMutationError::from)?;
        tx.insert_chain_op_publish(&op_hash, None, None, withhold_publish)
            .await
            .map_err(StateMutationError::from)?;
        tx.commit().await.map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Test-only: insert an entry into the store, routing a private entry to
    /// the `PrivateEntry` table (owned by `private_author`) and a public entry
    /// to the `Entry` table. Mirrors the entry write in the flush path so that
    /// store reads which resolve a full record — e.g.
    /// [`current_countersigning_session`](DhtStore::current_countersigning_session) —
    /// can find the entry of a hand-authored op inserted via
    /// [`test_insert_authored_chain_op`](DhtStore::test_insert_authored_chain_op).
    pub async fn test_insert_entry(
        &self,
        entry_hash: &holo_hash::EntryHash,
        entry: &holochain_types::prelude::Entry,
        private_author: Option<&AgentPubKey>,
    ) -> StateMutationResult<()> {
        match private_author {
            Some(author) => self
                .db
                .insert_private_entry(entry_hash, author, entry)
                .await
                .map_err(StateMutationError::from)?,
            None => self
                .db
                .insert_entry(entry_hash, entry)
                .await
                .map_err(StateMutationError::from)?,
        }
        Ok(())
    }

    /// Test-only: read the `last_publish_time` recorded for a chain op.
    pub async fn test_chain_op_publish_time(
        &self,
        op_hash: &DhtOpHash,
    ) -> DhtStoreResult<Option<Timestamp>> {
        let row = self
            .db
            .as_ref()
            .get_chain_op_publish(op_hash.clone())
            .await?;
        Ok(row.and_then(|r| r.last_publish_time.map(Timestamp::from_micros)))
    }

    /// Test-only: insert a `WarrantPublish` row recording `last_publish_time`
    /// for the given warrant.
    pub async fn test_insert_warrant_publish(
        &self,
        warrant_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
    ) -> StateMutationResult<()> {
        self.db
            .insert_warrant_publish(warrant_hash, last_publish_time)
            .await
            .map_err(StateMutationError::from)?;
        Ok(())
    }

    /// Test-only: overwrite the full publish state of an existing
    /// `ChainOpPublish` row. A `None` column is stored as SQL `NULL`; in
    /// particular `receipts_complete = None` means "receipts not complete"
    /// (the publish queue treats `receipts_complete IS NULL` as eligible).
    pub async fn test_set_chain_op_publish(
        &self,
        op_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
        receipts_complete: Option<bool>,
        withhold_publish: Option<bool>,
    ) -> DhtStoreResult<()> {
        sqlx::query(
            "UPDATE ChainOpPublish
             SET last_publish_time = ?, receipts_complete = ?, withhold_publish = ?
             WHERE op_hash = ?",
        )
        .bind(last_publish_time.map(|t| t.as_micros()))
        .bind(receipts_complete.map(|b| b as i64))
        .bind(withhold_publish.map(|b| b as i64))
        .bind(op_hash.get_raw_36())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Test-only: overwrite the `seq` of each of the given actions. Used to
    /// corrupt a source chain for sys-validation tests. Returns the number of
    /// `Action` rows updated.
    #[cfg(feature = "test_utils")]
    pub async fn test_set_action_seq(
        &self,
        action_hashes: &[ActionHash],
        seq: u32,
    ) -> DhtStoreResult<usize> {
        let mut updated = 0usize;
        for hash in action_hashes {
            let result = sqlx::query("UPDATE Action SET seq = ? WHERE hash = ?")
                .bind(seq as i64)
                .bind(hash.get_raw_36())
                .execute(self.db.pool())
                .await?;
            updated += result.rows_affected() as usize;
        }
        Ok(updated)
    }
}

pub(crate) mod action_indexes;
mod cache;
mod reads;
mod sync_reads;

#[cfg(test)]
mod tests;
