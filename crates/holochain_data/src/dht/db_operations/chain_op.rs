//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainOp` table.

use super::super::inner::chain_op::{self, InsertChainOp};
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

    pub async fn get_chain_ops_for_action(
        &self,
        action_hash: ActionHash,
    ) -> sqlx::Result<Vec<ChainOpRow>> {
        chain_op::get_chain_ops_for_action(self.pool(), action_hash).await
    }
}
