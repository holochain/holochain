//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `LimboWarrant` table.

use super::super::inner::limbo_warrant::{self, InsertLimboWarrant};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::LimboWarrantRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl TxWrite<Dht> {
    pub async fn insert_limbo_warrant(&mut self, w: InsertLimboWarrant<'_>) -> sqlx::Result<()> {
        limbo_warrant::insert_limbo_warrant(self.conn_mut(), w).await
    }

    pub async fn delete_limbo_warrant(&mut self, hash: DhtOpHash) -> sqlx::Result<()> {
        limbo_warrant::delete_limbo_warrant(self.conn_mut(), hash).await
    }

    /// Set the system-validation status for the given warrant. Returns the number of rows updated.
    pub async fn set_limbo_warrant_sys_validation_status(
        &mut self,
        hash: &DhtOpHash,
        status: Option<i64>,
    ) -> sqlx::Result<u64> {
        limbo_warrant::set_sys_validation_status(self.conn_mut(), hash, status).await
    }

    /// Record when validation was abandoned for the given warrant. Returns the number of rows updated.
    pub async fn set_limbo_warrant_abandoned_at(
        &mut self,
        hash: &DhtOpHash,
        when: Timestamp,
    ) -> sqlx::Result<u64> {
        limbo_warrant::set_abandoned_at(self.conn_mut(), hash, when).await
    }
}

impl TxRead<Dht> {
    pub async fn get_limbo_warrant(
        &mut self,
        hash: DhtOpHash,
    ) -> sqlx::Result<Option<LimboWarrantRow>> {
        limbo_warrant::get_limbo_warrant(self.conn_mut(), hash).await
    }

    pub async fn limbo_warrants_pending_sys_validation(
        &mut self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboWarrantRow>> {
        limbo_warrant::limbo_warrants_pending_sys_validation(self.conn_mut(), limit).await
    }

    pub async fn limbo_warrants_ready_for_integration(
        &mut self,
        limit: u32,
    ) -> sqlx::Result<Vec<LimboWarrantRow>> {
        limbo_warrant::limbo_warrants_ready_for_integration(self.conn_mut(), limit).await
    }
}
