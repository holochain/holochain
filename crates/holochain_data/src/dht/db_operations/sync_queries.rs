//! `DbRead<Dht>` API for the DHT-sync cross-table reads.

use super::super::inner::sync_queries::{self, ArcBounds};
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::{
    DumpChainOpRow, K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow,
    K2WarrantForWireRow,
};
use holo_hash::AgentPubKey;
#[cfg(any(test, feature = "inspection"))]
use holo_hash::{AnyLinkableHash, DhtOpHash};

impl DbRead<Dht> {
    /// `(hash, basis, size)` for every integrated, locally-validated op
    /// whose authored timestamp falls in `[t_start_micros, t_end_micros)`.
    pub async fn op_hashes_in_time_slice(
        &self,
        arc_start: u32,
        arc_end: u32,
        t_start_micros: i64,
        t_end_micros: i64,
    ) -> sqlx::Result<Vec<K2OpHashRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::op_hashes_in_time_slice(
            &mut *conn,
            ArcBounds {
                start: arc_start,
                end: arc_end,
            },
            t_start_micros,
            t_end_micros,
        )
        .await
    }

    /// Up to `limit` ops with `when_integrated >= t_min_micros`, ordered by
    /// integration time ascending. K2 gossip "since" cursor.
    pub async fn op_ids_since_time_batch(
        &self,
        arc_start: u32,
        arc_end: u32,
        t_min_micros: i64,
        limit: u32,
    ) -> sqlx::Result<Vec<K2OpIdSinceRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::op_ids_since_time_batch(
            &mut *conn,
            ArcBounds {
                start: arc_start,
                end: arc_end,
            },
            t_min_micros,
            limit,
        )
        .await
    }

    /// Subset of `op_hashes` we hold in limbo or as locally-validated
    /// integrated ops (cache-only copies excluded), with their basis hashes.
    pub async fn check_op_hashes_present(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2OpPresentRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::check_op_hashes_present(&mut *conn, op_hashes).await
    }

    /// Full chain-op rows (joined with `Action` and optional `Entry`) for
    /// the given op hashes, filtered to `locally_validated = 1`.
    pub async fn get_chain_ops_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2ChainOpForWireRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::get_chain_ops_for_wire(&mut *conn, op_hashes).await
    }

    /// Full warrant rows for the given op hashes.
    pub async fn get_warrants_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2WarrantForWireRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::get_warrants_for_wire(&mut *conn, op_hashes).await
    }

    /// Earliest authored timestamp across both tables within the arc, or
    /// `None` if no rows match.
    pub async fn earliest_authored_timestamp_in_arc(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> sqlx::Result<Option<i64>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::earliest_authored_timestamp_in_arc(
            &mut *conn,
            ArcBounds {
                start: arc_start,
                end: arc_end,
            },
        )
        .await
    }

    /// Total count of every integrated op + warrant (no `locally_validated`
    /// filter).
    pub async fn count_integrated_ops(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_integrated_ops(&mut *conn).await? as u64)
    }

    /// `(validation_limbo, integration_limbo, integrated)` counts for the
    /// integration-state report. Integrated counts locally-validated
    /// `ChainOp` rows (cache-only copies excluded) plus all `WarrantOp` rows.
    pub async fn limbo_state_counts(&self) -> sqlx::Result<(i64, i64, i64)> {
        let mut conn = self.timed_conn().await?;
        sync_queries::limbo_state_counts(&mut *conn).await
    }

    /// Count of integrated, locally-validated `ChainOp` rows that passed
    /// validation (rejected and GET-cached ops excluded).
    #[cfg(any(test, feature = "inspection"))]
    pub async fn count_valid_integrated_ops(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_valid_integrated_ops(&mut *conn).await? as u64)
    }

    /// Count of `LimboChainOp` rows that passed both sys- and app-validation
    /// but are not yet integrated.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn count_valid_not_integrated_ops(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_valid_not_integrated_ops(&mut *conn).await? as u64)
    }

    /// Count of not-yet-integrated chain ops authored by `author`.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn count_pending_ops_for_author(&self, author: &AgentPubKey) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_pending_ops_for_author(&mut *conn, author).await? as u64)
    }

    /// Hashes of integrated, rejected chain ops (GET-cached copies excluded).
    #[cfg(any(test, feature = "inspection"))]
    pub async fn rejected_integrated_op_hashes(&self) -> sqlx::Result<Vec<DhtOpHash>> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::rejected_integrated_op_hashes(&mut *conn)
            .await?
            .into_iter()
            .map(DhtOpHash::from_raw_36)
            .collect())
    }

    /// Total count of every op (integrated and limbo) held in this DHT store.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn count_all_ops(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_all_ops(&mut *conn).await? as u64)
    }

    /// Whether the integrated chain op `op_hash` requires a validation receipt.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn op_requires_receipt(&self, op_hash: &DhtOpHash) -> sqlx::Result<bool> {
        let mut conn = self.timed_conn().await?;
        sync_queries::op_requires_receipt(&mut *conn, op_hash).await
    }

    /// Whether `op_hash` is present in the limbo (not-yet-integrated) chain ops.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn limbo_op_exists(&self, op_hash: &DhtOpHash) -> sqlx::Result<bool> {
        let mut conn = self.timed_conn().await?;
        sync_queries::limbo_op_exists(&mut *conn, op_hash).await
    }

    /// Hashes of limbo chain ops flagged as requiring a validation receipt.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn limbo_op_hashes_requiring_receipt(&self) -> sqlx::Result<Vec<DhtOpHash>> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::limbo_op_hashes_requiring_receipt(&mut *conn)
            .await?
            .into_iter()
            .map(DhtOpHash::from_raw_36)
            .collect())
    }

    /// Hashes of integrated chain ops with the given DHT `basis`.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn get_ops_at_basis(&self, basis: &AnyLinkableHash) -> sqlx::Result<Vec<DhtOpHash>> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::get_ops_at_basis(&mut *conn, basis)
            .await?
            .into_iter()
            .map(DhtOpHash::from_raw_36)
            .collect())
    }

    /// Count of rows in the public `Entry` table.
    #[cfg(any(test, feature = "inspection"))]
    pub async fn count_entries(&self) -> sqlx::Result<u64> {
        let mut conn = self.timed_conn().await?;
        Ok(sync_queries::count_entries(&mut *conn).await? as u64)
    }

    /// Integrated chain-op rows for the integration dump, paginated forward
    /// from the `(when_integrated, op_hash)` cursor `after` (`None` from the
    /// start, which yields the full set).
    pub async fn integrated_chain_ops_for_dump(
        &self,
        after: Option<(i64, &[u8])>,
    ) -> sqlx::Result<Vec<DumpChainOpRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::integrated_chain_ops_for_dump(&mut *conn, after).await
    }

    /// Limbo chain-op rows for the integration dump. `ready` selects the
    /// integration-limbo subset; `!ready` selects the validation-limbo subset.
    pub async fn limbo_chain_ops_for_dump(
        &self,
        ready: bool,
    ) -> sqlx::Result<Vec<K2ChainOpForWireRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::limbo_chain_ops_for_dump(&mut *conn, ready).await
    }

    /// Chain-op rows authored and shared by `author`, joined for wire
    /// reconstruction. Excludes private `StoreEntry` ops so private entries
    /// never leak into the published set.
    pub async fn ops_to_publish_for_wire(
        &self,
        author: &AgentPubKey,
    ) -> sqlx::Result<Vec<K2ChainOpForWireRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::ops_to_publish_for_wire(&mut *conn, author).await
    }

    /// Every integrated warrant row for the integration dump.
    pub async fn integrated_warrants_for_dump(&self) -> sqlx::Result<Vec<K2WarrantForWireRow>> {
        let mut conn = self.timed_conn().await?;
        sync_queries::integrated_warrants_for_dump(&mut *conn).await
    }
}
