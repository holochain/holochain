//! `DbWrite<Dht>` API for moving a cached `ChainOp` row back into
//! `LimboChainOp`.

use super::super::inner::move_to_limbo;
use crate::handles::DbWrite;
use crate::kind::Dht;
use holo_hash::ActionHash;

impl DbWrite<Dht> {
    /// See [`crate::dht::inner::move_to_limbo::move_chain_op_to_limbo`].
    ///
    /// Wraps the atomic move in a transaction. Returns `true` if a cached
    /// `ChainOp` row (with `locally_validated = 0`) matching `(action_hash,
    /// op_type)` was moved into `LimboChainOp`, or `false` if no such row
    /// exists.
    pub async fn move_chain_op_to_limbo(
        &self,
        action_hash: &ActionHash,
        op_type: i64,
    ) -> sqlx::Result<bool> {
        let mut tx = self.begin().await?;
        let moved =
            move_to_limbo::move_chain_op_to_limbo(tx.conn_mut(), action_hash, op_type).await?;
        tx.commit().await?;
        Ok(moved)
    }
}
