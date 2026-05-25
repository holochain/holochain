//! `DbRead<Dht>` API for the DHT-sync cross-table reads.

use super::super::inner::sync_queries::{self, ArcBounds};
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::{
    K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow, K2WarrantForWireRow,
};

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
        sync_queries::op_hashes_in_time_slice(
            self.pool(),
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
        sync_queries::op_ids_since_time_batch(
            self.pool(),
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
        sync_queries::check_op_hashes_present(self.pool(), op_hashes).await
    }

    /// Full chain-op rows (joined with `Action` and optional `Entry`) for
    /// the given op hashes, filtered to `locally_validated = 1`.
    pub async fn get_chain_ops_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2ChainOpForWireRow>> {
        sync_queries::get_chain_ops_for_wire(self.pool(), op_hashes).await
    }

    /// Full warrant rows for the given op hashes.
    pub async fn get_warrants_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2WarrantForWireRow>> {
        sync_queries::get_warrants_for_wire(self.pool(), op_hashes).await
    }

    /// Earliest authored timestamp across both tables within the arc, or
    /// `None` if no rows match.
    pub async fn earliest_authored_timestamp_in_arc(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> sqlx::Result<Option<i64>> {
        sync_queries::earliest_authored_timestamp_in_arc(
            self.pool(),
            ArcBounds {
                start: arc_start,
                end: arc_end,
            },
        )
        .await
    }

    /// Total count of every integrated op + warrant (no `locally_validated`
    /// filter).
    pub async fn count_integrated_ops(&self) -> sqlx::Result<i64> {
        sync_queries::count_integrated_ops(self.pool()).await
    }
}
