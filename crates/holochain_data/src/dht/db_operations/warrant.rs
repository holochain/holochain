//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Warrant` table.

use super::super::inner::warrant::{self, InsertWarrant};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::WarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};

impl DbWrite<Dht> {
    pub async fn insert_warrant(&self, w: InsertWarrant<'_>) -> sqlx::Result<()> {
        warrant::insert_warrant(self.pool(), w).await
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
