//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `CapGrant` table.

use super::super::inner::cap_grant;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::CapGrantRow;
use holo_hash::{ActionHash, AgentPubKey};

impl DbWrite<Dht> {
    pub async fn insert_cap_grant(
        &self,
        action_hash: &ActionHash,
        cap_access: i64,
        tag: Option<&str>,
    ) -> sqlx::Result<()> {
        cap_grant::insert_cap_grant(self.pool(), action_hash, cap_access, tag).await
    }
}

impl DbRead<Dht> {
    pub async fn get_cap_grants_by_access(
        &self,
        author: AgentPubKey,
        cap_access: i64,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        cap_grant::get_cap_grants_by_access(self.pool(), author, cap_access).await
    }

    pub async fn get_cap_grants_by_tag(
        &self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        cap_grant::get_cap_grants_by_tag(self.pool(), author, tag).await
    }
}
