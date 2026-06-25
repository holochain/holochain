//! `DhtStore` methods that back the Kitsune2 `OpStore` (gossip/sync).
//!
//! These are the cross-table op reads K2 issues during gossip plus the
//! slice-hash cache access. The reads are generic over any handle that
//! borrows a read pool, so both `DhtStoreRead` and the writable
//! `DhtStore<DbWrite<Dht>>` expose them; the slice-hash write needs
//! `DbWrite`. All `sqlx::Error`s map into `StateQueryError`.

use holochain_data::kind::Dht;
use holochain_data::DbWrite;
use holochain_types::prelude::AgentPubKey;
use holochain_types::prelude::Timestamp;

use super::DhtStore;
use crate::mutations::{StateMutationError, StateMutationResult};

impl<Db> DhtStore<Db>
where
    Db: AsRef<holochain_data::DbRead<Dht>>,
{
    /// `(op_hash, basis, size)` triples for every integrated, locally-
    /// validated op whose authored timestamp falls in `[start, end)`.
    /// Used by K2 `retrieve_op_hashes_in_time_slice`.
    pub async fn op_hashes_in_time_slice(
        &self,
        arc_start: u32,
        arc_end: u32,
        start: Timestamp,
        end: Timestamp,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2OpHashRow>> {
        self.db
            .as_ref()
            .op_hashes_in_time_slice(arc_start, arc_end, start.as_micros(), end.as_micros())
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Up to `limit` `(hash, basis, when_integrated, size)` tuples with
    /// `when_integrated >= t_min`. Used by K2 `retrieve_op_ids_bounded`'s
    /// inner paging loop.
    pub async fn op_ids_since_time_batch(
        &self,
        arc_start: u32,
        arc_end: u32,
        t_min: Timestamp,
        limit: u32,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2OpIdSinceRow>> {
        self.db
            .as_ref()
            .op_ids_since_time_batch(arc_start, arc_end, t_min.as_micros(), limit)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Subset of `op_hashes` we hold in limbo or as locally-validated
    /// integrated ops (cache-only copies excluded), with their basis hashes.
    /// Used by K2 `filter_out_existing_ops`.
    pub async fn check_op_hashes_present(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2OpPresentRow>> {
        self.db
            .as_ref()
            .check_op_hashes_present(op_hashes)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Joined chain-op rows (ChainOp joined with Action, left-joined with
    /// Entry) for wire reconstruction. Filtered to `locally_validated = 1`.
    pub async fn get_chain_ops_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2ChainOpForWireRow>> {
        self.db
            .as_ref()
            .get_chain_ops_for_wire(op_hashes)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Warrant rows for wire reconstruction.
    pub async fn get_warrants_for_wire(
        &self,
        op_hashes: &[Vec<u8>],
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2WarrantForWireRow>> {
        self.db
            .as_ref()
            .get_warrants_for_wire(op_hashes)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Earliest authored timestamp in the arc, across both `ChainOp`
    /// (joined with `Action`) and `Warrant`. `None` if no rows match.
    pub async fn earliest_authored_timestamp_in_arc(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> crate::query::StateQueryResult<Option<Timestamp>> {
        let micros = self
            .db
            .as_ref()
            .earliest_authored_timestamp_in_arc(arc_start, arc_end)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)?;
        Ok(micros.map(Timestamp::from_micros))
    }

    /// `(validation_limbo, integration_limbo, integrated)` counts for the
    /// integration-state report, each as `usize`. `integrated` counts
    /// locally-validated `ChainOp` rows (GET-cached copies excluded) plus all
    /// `WarrantOp` rows; `integration_limbo` is the limbo subset ready for
    /// integration and `validation_limbo` the remainder still in validation.
    pub async fn limbo_state_counts(
        &self,
    ) -> crate::query::StateQueryResult<(usize, usize, usize)> {
        let (validation_limbo, integration_limbo, integrated) = self
            .db
            .as_ref()
            .limbo_state_counts()
            .await
            .map_err(crate::query::StateQueryError::Sqlx)?;
        Ok((
            validation_limbo.max(0) as usize,
            integration_limbo.max(0) as usize,
            integrated.max(0) as usize,
        ))
    }

    /// Integrated chain-op rows for the integration dump, paginated forward
    /// from the `(when_integrated, op_hash)` cursor `after` (`None` from the
    /// start, which yields the full set — also how the consistency harness
    /// reads everything).
    pub async fn integrated_chain_ops_for_dump(
        &self,
        after: Option<(i64, &[u8])>,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::DumpChainOpRow>> {
        self.db
            .as_ref()
            .integrated_chain_ops_for_dump(after)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Limbo chain-op rows for the integration dump. `ready` selects the
    /// integration-limbo subset; `!ready` selects the validation-limbo subset.
    pub async fn limbo_chain_ops_for_dump(
        &self,
        ready: bool,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2ChainOpForWireRow>> {
        self.db
            .as_ref()
            .limbo_chain_ops_for_dump(ready)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Every integrated warrant row for the integration dump.
    pub async fn integrated_warrants_for_dump(
        &self,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2WarrantForWireRow>> {
        self.db
            .as_ref()
            .integrated_warrants_for_dump()
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Chain-op rows authored and shared by `author`, joined for wire
    /// reconstruction. Excludes private `StoreEntry` ops so private entries
    /// never leak into the published set. Used by the consistency-check
    /// harness to gather a cell's published ops.
    pub async fn ops_to_publish_for_wire(
        &self,
        author: &AgentPubKey,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::K2ChainOpForWireRow>> {
        self.db
            .as_ref()
            .ops_to_publish_for_wire(author)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Total integrated op + warrant count (no `locally_validated` filter
    /// — preserves the old DHT+cache combined count).
    pub async fn total_integrated_op_count(&self) -> crate::query::StateQueryResult<u64> {
        let n = self
            .db
            .as_ref()
            .count_integrated_ops()
            .await
            .map_err(crate::query::StateQueryError::Sqlx)?;
        Ok(n)
    }

    /// Number of stored slices for the arc, or 0 if none.
    ///
    /// K2 assigns slice indices consecutively from 0, so the count is the
    /// highest stored index + 1 (matching the kitsune2 reference op-store);
    /// the query layer adds the one. Returned to K2 as `slice_hash_count`.
    pub async fn slice_hash_count(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> crate::query::StateQueryResult<u64> {
        self.db
            .as_ref()
            .slice_hash_count(arc_start, arc_end)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Single stored slice hash, if any.
    pub async fn get_slice_hash(
        &self,
        arc_start: u32,
        arc_end: u32,
        slice_index: u64,
    ) -> crate::query::StateQueryResult<Option<Vec<u8>>> {
        self.db
            .as_ref()
            .get_slice_hash(arc_start, arc_end, slice_index)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }

    /// Every `(slice_index, hash)` pair stored for the arc.
    pub async fn get_slice_hashes(
        &self,
        arc_start: u32,
        arc_end: u32,
    ) -> crate::query::StateQueryResult<Vec<holochain_data::models::dht::SliceHashIndexedRow>> {
        self.db
            .as_ref()
            .get_slice_hashes(arc_start, arc_end)
            .await
            .map_err(crate::query::StateQueryError::Sqlx)
    }
}

impl DhtStore<DbWrite<Dht>> {
    /// Insert or replace the slice hash for `(arc, slice_index)`. K2
    /// `store_slice_hash`.
    pub async fn store_slice_hash(
        &self,
        arc_start: u32,
        arc_end: u32,
        slice_index: u64,
        hash: &[u8],
    ) -> StateMutationResult<()> {
        self.db
            .insert_slice_hash(arc_start, arc_end, slice_index, hash)
            .await
            .map_err(StateMutationError::from)?;
        Ok(())
    }
}
