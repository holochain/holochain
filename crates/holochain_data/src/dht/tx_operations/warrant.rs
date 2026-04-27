//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `Warrant` table.

use super::super::inner::warrant::{self, InsertWarrant};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::WarrantRow;
use holo_hash::{AgentPubKey, DhtOpHash};

impl TxWrite<Dht> {
    pub async fn insert_warrant(&mut self, w: InsertWarrant<'_>) -> sqlx::Result<()> {
        warrant::insert_warrant(self.conn_mut(), w).await
    }
}

impl TxRead<Dht> {
    pub async fn get_warrant(&mut self, hash: DhtOpHash) -> sqlx::Result<Option<WarrantRow>> {
        warrant::get_warrant(self.conn_mut(), hash).await
    }

    pub async fn get_warrants_by_warrantee(
        &mut self,
        warrantee: AgentPubKey,
    ) -> sqlx::Result<Vec<WarrantRow>> {
        warrant::get_warrants_by_warrantee(self.conn_mut(), warrantee).await
    }
}
