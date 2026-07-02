//! `DbRead<Dht>` API for op-existence checks.

use super::super::inner::op_exists;
use crate::handles::DbRead;
use crate::kind::Dht;
use holo_hash::DhtOpHash;

impl DbRead<Dht> {
    /// Returns `true` if the given op hash appears in any op-bearing
    /// table (`ChainOp`, `LimboChainOp`, `Warrant`, `LimboWarrant`).
    pub async fn op_exists(&self, hash: &DhtOpHash) -> sqlx::Result<bool> {
        let mut conn = self.timed_conn().await?;
        op_exists::op_exists(&mut *conn, hash).await
    }

    /// For each input hash, return whether it appears in any
    /// op-bearing table. Result aligns 1:1 with the input.
    pub async fn op_hashes_present(&self, hashes: &[DhtOpHash]) -> sqlx::Result<Vec<bool>> {
        let mut conn = self.timed_conn().await?;
        op_exists::op_hashes_present(&mut conn, hashes).await
    }
}
