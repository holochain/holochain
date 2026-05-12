//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `ChainOp` table.

use super::super::inner::chain_op::{self, InsertChainOp};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::ChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};
use holochain_integrity_types::dht_v2::OpValidity;

impl TxWrite<Dht> {
    pub async fn insert_chain_op(&mut self, op: InsertChainOp<'_>) -> sqlx::Result<()> {
        chain_op::insert_chain_op(self.conn_mut(), op).await
    }

    /// Update `validation_status` for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_validation_status(
        &mut self,
        op_hash: &DhtOpHash,
        validation_status: OpValidity,
    ) -> sqlx::Result<u64> {
        chain_op::set_validation_status(self.conn_mut(), op_hash, validation_status).await
    }
}

impl TxRead<Dht> {
    pub async fn get_chain_op(&mut self, hash: DhtOpHash) -> sqlx::Result<Option<ChainOpRow>> {
        chain_op::get_chain_op(self.conn_mut(), hash).await
    }

    pub async fn get_chain_ops_by_basis(
        &mut self,
        basis: AnyDhtHash,
    ) -> sqlx::Result<Vec<ChainOpRow>> {
        chain_op::get_chain_ops_by_basis(self.conn_mut(), basis).await
    }

    pub async fn get_chain_ops_for_action(
        &mut self,
        action_hash: ActionHash,
    ) -> sqlx::Result<Vec<ChainOpRow>> {
        chain_op::get_chain_ops_for_action(self.conn_mut(), action_hash).await
    }
}
