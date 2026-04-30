//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `LimboChainOp` table.

use super::super::inner::limbo_chain_op::{self, InsertLimboChainOp};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::LimboChainOpRow;
use holo_hash::DhtOpHash;

impl DbWrite<Dht> {
    pub async fn insert_limbo_chain_op(&self, op: InsertLimboChainOp<'_>) -> sqlx::Result<()> {
        limbo_chain_op::insert_limbo_chain_op(self.pool(), op).await
    }

    pub async fn delete_limbo_chain_op(&self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_chain_op::delete_limbo_chain_op(self.pool(), hash).await
    }
}

impl DbRead<Dht> {
    pub async fn get_limbo_chain_op(
        &self,
        hash: DhtOpHash,
    ) -> sqlx::Result<Option<LimboChainOpRow>> {
        limbo_chain_op::get_limbo_chain_op(self.pool(), hash).await
    }

    pub async fn limbo_chain_ops_pending_sys_validation(
        &self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_pending_sys_validation(self.pool(), limit).await
    }

    pub async fn limbo_chain_ops_pending_app_validation(
        &self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_pending_app_validation(self.pool(), limit).await
    }

    pub async fn limbo_chain_ops_ready_for_integration(
        &self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboChainOpRow>> {
        limbo_chain_op::limbo_chain_ops_ready_for_integration(self.pool(), limit).await
    }
}
