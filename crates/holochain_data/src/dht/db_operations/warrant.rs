//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Warrant` + `WarrantOp` tables.

use super::super::inner::warrant::{self, InsertWarrant};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::WarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};

impl DbWrite<Dht> {
    /// Insert an integrated warrant atomically into `Warrant` + `WarrantOp`.
    pub async fn insert_warrant(&self, w: InsertWarrant<'_>) -> sqlx::Result<()> {
        let mut tx = self.begin().await?;
        warrant::insert_warrant(tx.conn_mut(), w).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl DbRead<Dht> {
    pub async fn get_warrant(&self, hash: DhtOpHash) -> sqlx::Result<Option<WarrantRow>> {
        warrant::get_warrant(self.pool(), hash).await
    }

    pub async fn get_warrants_by_warrantee(
        &self,
        warrantee: AgentPubKey,
    ) -> sqlx::Result<Vec<WarrantRow>> {
        warrant::get_warrants_by_warrantee(self.pool(), warrantee).await
    }
}
