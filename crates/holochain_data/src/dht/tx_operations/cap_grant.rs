//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `CapGrant` table.

use super::super::inner::cap_grant;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::CapGrantRow;
use holo_hash::{ActionHash, AgentPubKey};

impl TxWrite<Dht> {
    pub async fn insert_cap_grant(
        &mut self,
        action_hash: &ActionHash,
        cap_access: i64,
        tag: Option<&str>,
    ) -> sqlx::Result<()> {
        cap_grant::insert_cap_grant(self.conn_mut(), action_hash, cap_access, tag).await
    }
}

impl TxRead<Dht> {
    pub async fn get_cap_grants_by_access(
        &mut self,
        author: AgentPubKey,
        cap_access: i64,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        cap_grant::get_cap_grants_by_access(self.conn_mut(), author, cap_access).await
    }

    pub async fn get_cap_grants_by_tag(
        &mut self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        cap_grant::get_cap_grants_by_tag(self.conn_mut(), author, tag).await
    }
}
