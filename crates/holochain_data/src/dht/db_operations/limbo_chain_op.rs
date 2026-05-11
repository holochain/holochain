//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `LimboChainOp` table.

use super::super::inner::limbo_chain_op::{self, InsertLimboChainOp};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::LimboChainOpRow;
use holo_hash::DhtOpHash;
use holochain_integrity_types::dht_v2::RecordValidity;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_limbo_chain_op(&self, op: InsertLimboChainOp<'_>) -> sqlx::Result<()> {
        limbo_chain_op::insert_limbo_chain_op(self.pool(), op).await
    }

    pub async fn delete_limbo_chain_op(&self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_chain_op::delete_limbo_chain_op(self.pool(), hash).await
    }

    /// Set the system-validation status for the given op. Returns the number of rows updated.
    pub async fn set_limbo_chain_op_sys_validation_status(
        &self,
        op_hash: &DhtOpHash,
        status: Option<i64>,
    ) -> sqlx::Result<u64> {
        limbo_chain_op::set_sys_validation_status(self.pool(), op_hash, status).await
    }

    /// Set the app-validation status for the given op. Returns the number of rows updated.
    pub async fn set_limbo_chain_op_app_validation_status(
        &self,
        op_hash: &DhtOpHash,
        status: Option<i64>,
    ) -> sqlx::Result<u64> {
        limbo_chain_op::set_app_validation_status(self.pool(), op_hash, status).await
    }

    /// Record when validation was abandoned for the given op. Returns the number of rows updated.
    pub async fn set_limbo_chain_op_abandoned_at(
        &self,
        op_hash: &DhtOpHash,
        when: Timestamp,
    ) -> sqlx::Result<u64> {
        limbo_chain_op::set_abandoned_at(self.pool(), op_hash, when).await
    }

    /// Set the `require_receipt` flag for the given op. Returns the number of rows updated.
    pub async fn set_limbo_chain_op_require_receipt(
        &self,
        op_hash: &DhtOpHash,
        require_receipt: bool,
    ) -> sqlx::Result<u64> {
        limbo_chain_op::set_require_receipt(self.pool(), op_hash, require_receipt).await
    }

    /// Atomically promote a `LimboChainOp` row to the `ChainOp` table.
    ///
    /// Begins a transaction, delegates to the inner promotion helper, and
    /// commits on success.  Returns `true` if the limbo row existed and was
    /// promoted, `false` if it did not exist.
    pub async fn promote_limbo_chain_op(
        &self,
        op_hash: &DhtOpHash,
        validation_status: RecordValidity,
        when_integrated: Timestamp,
    ) -> sqlx::Result<bool> {
        let mut tx = self.begin().await?;
        let result = limbo_chain_op::promote_to_chain_op(
            tx.conn_mut(),
            op_hash,
            validation_status,
            when_integrated,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
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
