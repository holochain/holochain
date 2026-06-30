//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainOp` table.

use super::super::inner::chain_op::{self, InsertChainOp, OpLocationRow, PendingReceiptRow};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::ChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};
use holochain_integrity_types::dht_v2::OpValidity;

impl DbWrite<Dht> {
    pub async fn insert_chain_op(&self, op: InsertChainOp<'_>) -> sqlx::Result<()> {
        chain_op::insert_chain_op(self.pool(), op).await
    }

    /// Update `validation_status` for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_validation_status(
        &self,
        op_hash: &DhtOpHash,
        validation_status: OpValidity,
    ) -> sqlx::Result<u64> {
        chain_op::set_validation_status(self.pool(), op_hash, validation_status).await
    }

    /// Clear the `require_receipt` flag on the `ChainOp` row for the given op.
    /// Returns the number of rows updated.
    pub async fn clear_chain_op_require_receipt(&self, op_hash: &DhtOpHash) -> sqlx::Result<u64> {
        chain_op::clear_require_receipt(self.pool(), op_hash).await
    }
}

impl DbRead<Dht> {
    pub async fn get_chain_op(&self, hash: DhtOpHash) -> sqlx::Result<Option<ChainOpRow>> {
        chain_op::get_chain_op(self.pool(), hash).await
    }

    pub async fn get_chain_ops_by_basis(&self, basis: AnyDhtHash) -> sqlx::Result<Vec<ChainOpRow>> {
        chain_op::get_chain_ops_by_basis(self.pool(), basis).await
    }

    /// `(hash, basis_hash, storage_center_loc)` for every integrated chain op.
    pub async fn integrated_op_locations(&self) -> sqlx::Result<Vec<OpLocationRow>> {
        chain_op::integrated_op_locations(self.pool()).await
    }

    pub async fn get_chain_ops_for_action(
        &self,
        action_hash: ActionHash,
    ) -> sqlx::Result<Vec<ChainOpRow>> {
        chain_op::get_chain_ops_for_action(self.pool(), action_hash).await
    }

    /// Terminal validation outcome of the chain op for `(action_hash,
    /// op_type)`, from `ChainOp` or `LimboChainOp`. See
    /// `chain_op::op_validation_outcome`.
    pub async fn op_validation_outcome(
        &self,
        action_hash: &ActionHash,
        op_type: i64,
    ) -> sqlx::Result<Option<i64>> {
        chain_op::op_validation_outcome(self.pool(), action_hash, op_type).await
    }

    /// Return integrated, validated `ChainOp` rows that still require a
    /// validation receipt to be sent.
    pub async fn pending_validation_receipts(&self) -> sqlx::Result<Vec<PendingReceiptRow>> {
        chain_op::pending_validation_receipts(self.pool()).await
    }
}
