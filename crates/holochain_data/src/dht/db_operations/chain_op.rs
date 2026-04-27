//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainOp` table.

use super::super::inner::chain_op::{self, InsertChainOp};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::ChainOpRow;
use holo_hash::{ActionHash, AnyDhtHash, DhtOpHash};

impl DbWrite<Dht> {
    pub async fn insert_chain_op(&self, op: InsertChainOp<'_>) -> sqlx::Result<()> {
        chain_op::insert_chain_op(self.pool(), op).await
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
