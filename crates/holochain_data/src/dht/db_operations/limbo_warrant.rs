//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `LimboWarrant` table.

use super::super::inner::limbo_warrant::{self, InsertLimboWarrant};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::LimboWarrantRow;
use holo_hash::DhtOpHash;

impl DbWrite<Dht> {
    pub async fn insert_limbo_warrant(&self, w: InsertLimboWarrant<'_>) -> sqlx::Result<()> {
        limbo_warrant::insert_limbo_warrant(self.pool(), w).await
    }

    pub async fn delete_limbo_warrant(&self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_warrant::delete_limbo_warrant(self.pool(), hash).await
    }
}

impl DbRead<Dht> {
    pub async fn get_limbo_warrant(
        &self,
        hash: DhtOpHash,
    ) -> sqlx::Result<Option<LimboWarrantRow>> {
        limbo_warrant::get_limbo_warrant(self.pool(), hash).await
    }

    pub async fn limbo_warrants_pending_sys_validation(
        &self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboWarrantRow>> {
        limbo_warrant::limbo_warrants_pending_sys_validation(self.pool(), limit).await
    }

    pub async fn limbo_warrants_ready_for_integration(
        &self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboWarrantRow>> {
        limbo_warrant::limbo_warrants_ready_for_integration(self.pool(), limit).await
    }
}
