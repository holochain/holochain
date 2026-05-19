//! `DbRead<Dht>` API for the K2 op-store cross-table reads.

use super::super::inner::k2_reads::{self, ArcBounds};
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::{
    K2ChainOpForWireRow, K2OpHashRow, K2OpIdSinceRow, K2OpPresentRow, K2WarrantForWireRow,
};

impl DbRead<Dht> {
    /// `(hash, basis, size)` for every integrated, locally-validated op
    /// whose authored timestamp falls in `[t_start_micros, t_end_micros)`.
    pub async fn k2_op_hashes_in_time_slice(
        &self,
        arc_start: u32,
        arc_end: u32,
        t_start_micros: i64,
        t_end_micros: i64,
    ) -> sqlx::Result<Vec<K2OpHashRow>> {
        k2_reads::op_hashes_in_time_slice(
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
    pub async fn k2_op_ids_since_time_batch(
        &self,
        arc_start: u32,
        arc_end: u32,
        t_min_micros: i64,
        limit: u32,
    ) -> sqlx::Result<Vec<K2OpIdSinceRow>> {
        k2_reads::op_ids_since_time_batch(
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

    /// Subset of `op_hashes` present in `ChainOp` (locally validated) or
    /// `Warrant`, with their basis hashes.
    pub async fn k2_check_op_hashes_present(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2OpPresentRow>> {
        k2_reads::check_op_hashes_present(self.pool(), op_hashes).await
    }

    /// Full chain-op rows (joined with `Action` and optional `Entry`) for
    /// the given op hashes, filtered to `locally_validated = 1`.
    pub async fn k2_get_chain_ops_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2ChainOpForWireRow>> {
        k2_reads::get_chain_ops_for_wire(self.pool(), op_hashes).await
    }

    /// Full warrant rows for the given op hashes.
    pub async fn k2_get_warrants_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> sqlx::Result<Vec<K2WarrantForWireRow>> {
        k2_reads::get_warrants_for_wire(self.pool(), op_hashes).await
    }

    /// Earliest authored timestamp across both tables within the arc, or
    /// `None` if no rows match.
    pub async fn k2_earliest_authored_timestamp_in_arc(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> sqlx::Result<Option<i64>> {
        k2_reads::earliest_authored_timestamp_in_arc(
            self.pool(),
            ArcBounds {
                start: arc_start,
                end: arc_end,
            },
        )
        .await
    }

    /// Total count of every integrated op + warrant (no `locally_validated`
    /// filter). Preserves the K2 `query_total_op_count` "everything we hold
    /// locally" semantic.
    pub async fn k2_count_integrated_ops(&self) -> sqlx::Result<i64> {
        k2_reads::count_integrated_ops(self.pool()).await
    }
}
