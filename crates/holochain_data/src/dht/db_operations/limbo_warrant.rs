//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `LimboWarrant` table.

use super::super::inner::limbo_warrant::{self, InsertLimboWarrant};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::LimboWarrantRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_limbo_warrant(&self, w: InsertLimboWarrant<'_>) -> sqlx::Result<()> {
        limbo_warrant::insert_limbo_warrant(self.pool(), w).await
    }

    pub async fn delete_limbo_warrant(&self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_warrant::delete_limbo_warrant(self.pool(), hash).await
    }

    /// Set the system-validation status for the given warrant. Returns the number of rows updated.
    pub async fn set_limbo_warrant_sys_validation_status(
        &self,
        hash: &DhtOpHash,
        status: Option<i64>,
    ) -> sqlx::Result<u64> {
        limbo_warrant::set_sys_validation_status(self.pool(), hash, status).await
    }

    /// Record when validation was abandoned for the given warrant. Returns the number of rows updated.
    pub async fn set_limbo_warrant_abandoned_at(
        &self,
        hash: &DhtOpHash,
        when: Timestamp,
    ) -> sqlx::Result<u64> {
        limbo_warrant::set_abandoned_at(self.pool(), hash, when).await
    }

    /// Atomically promote a `LimboWarrant` row to the `Warrant` table.
    ///
    /// Begins a transaction, delegates to the inner promotion helper, and
    /// commits on success.  Returns `true` if the limbo row existed and was
    /// promoted, `false` if it did not exist.
    pub async fn promote_limbo_warrant(&self, hash: &DhtOpHash) -> sqlx::Result<bool> {
        let mut tx = self.begin().await?;
        let result = limbo_warrant::promote_to_warrant(tx.conn_mut(), hash).await?;
        tx.commit().await?;
        Ok(result)
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
