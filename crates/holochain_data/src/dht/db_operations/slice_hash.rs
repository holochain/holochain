//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `SliceHash` table.

use super::super::inner::slice_hash;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::SliceHashIndexedRow;

impl DbWrite<Dht> {
    /// Insert or replace the slice hash for `(arc_start, arc_end, slice_index)`.
    pub async fn insert_slice_hash(
        &self,
        arc_start: u32,
        arc_end: u32,
        slice_index: u64,
        hash: &[u8],
    ) -> sqlx::Result<()> {
        slice_hash::insert_slice_hash(self.pool(), arc_start, arc_end, slice_index, hash).await
    }
}

impl DbRead<Dht> {
    /// Highest stored slice index for the arc, or 0 if none.
    pub async fn max_slice_index(&self, arc_start: u32, arc_end: u32) -> sqlx::Result<u64> {
        slice_hash::max_slice_index(self.pool(), arc_start, arc_end).await
    }

    /// Single slice hash, if any.
    pub async fn get_slice_hash(
        &self,
        arc_start: u32,
        arc_end: u32,
        slice_index: u64,
    ) -> sqlx::Result<Option<Vec<u8>>> {
        slice_hash::get_slice_hash(self.pool(), arc_start, arc_end, slice_index).await
    }

    /// Every `(slice_index, hash)` pair stored for the arc.
    pub async fn get_slice_hashes(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> sqlx::Result<Vec<SliceHashIndexedRow>> {
        slice_hash::get_slice_hashes(self.pool(), arc_start, arc_end).await
    }
}
