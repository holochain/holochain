//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `LimboChainOp` table.

use super::super::inner::limbo_chain_op::{self, InsertLimboChainOp};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::LimboChainOpRow;
use holo_hash::DhtOpHash;

impl TxWrite<Dht> {
    pub async fn insert_limbo_chain_op(&mut self, op: InsertLimboChainOp<'_>) -> sqlx::Result<()> {
        limbo_chain_op::insert_limbo_chain_op(self.conn_mut(), op).await
    }

    pub async fn delete_limbo_chain_op(&mut self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_chain_op::delete_limbo_chain_op(self.conn_mut(), hash).await
    }
}

impl TxRead<Dht> {
    pub async fn get_limbo_chain_op(
        &mut self,
        hash: DhtOpHash,
    ) -> sqlx::Result<Option<LimboChainOpRow>> {
        limbo_chain_op::get_limbo_chain_op(self.conn_mut(), hash).await
    }

    pub async fn limbo_chain_ops_pending_sys_validation(
        &mut self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_pending_sys_validation(self.conn_mut(), limit).await
    }

    pub async fn limbo_chain_ops_pending_app_validation(
        &mut self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_pending_app_validation(self.conn_mut(), limit).await
    }

    pub async fn limbo_chain_ops_ready_for_integration(
        &mut self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_ready_for_integration(self.conn_mut(), limit).await
    }
}
