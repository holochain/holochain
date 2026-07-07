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
        let mut conn = self.timed_conn().await?;
        warrant::get_warrant(&mut *conn, hash).await
    }

    pub async fn get_warrants_by_warrantee(
        &self,
        warrantee: AgentPubKey,
    ) -> sqlx::Result<Vec<WarrantRow>> {
        let mut conn = self.timed_conn().await?;
        warrant::get_warrants_by_warrantee(&mut *conn, warrantee).await
    }

    /// Warrants authored by `author` (the warrant issuer).
    pub async fn get_warrants_by_author(
        &self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<WarrantRow>> {
        let mut conn = self.timed_conn().await?;
        warrant::get_warrants_by_author(&mut *conn, author).await
    }

    /// Terminal validation status of an integrated warrant op, or `None`.
    pub async fn warrant_op_validation_status(
        &self,
        op_hash: &DhtOpHash,
    ) -> sqlx::Result<Option<i64>> {
        let mut conn = self.timed_conn().await?;
        warrant::warrant_op_validation_status(&mut *conn, op_hash).await
    }

    /// Serialized `WarrantProof`s of pending-or-valid warrants against
    /// `warrantee`. See
    /// `warrant::pending_or_valid_warrant_proofs_by_warrantee`.
    pub async fn pending_or_valid_warrant_proofs_by_warrantee(
        &self,
        warrantee: &AgentPubKey,
    ) -> sqlx::Result<Vec<Vec<u8>>> {
        let mut conn = self.timed_conn().await?;
        warrant::pending_or_valid_warrant_proofs_by_warrantee(&mut *conn, warrantee).await
    }
}
