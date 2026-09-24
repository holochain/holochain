//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `CapGrant` table.

use super::super::inner::cap_grant;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::CapGrantRow;
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_integrity_types::capability::CapGrant;

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
    /// Reads the author's capability grant stored as a private entry.
    ///
    /// Only the author's `PrivateEntry` row for `entry_hash` is consulted; a
    /// same-hash public `Entry` or another agent's private entry is never
    /// returned. See `inner::cap_grant::get_cap_grant_entry`.
    pub async fn get_cap_grant_entry(
        &self,
        entry_hash: &EntryHash,
        author: &AgentPubKey,
    ) -> sqlx::Result<Option<CapGrant>> {
        let mut conn = self.timed_conn().await?;
        cap_grant::get_cap_grant_entry(&mut *conn, entry_hash, author).await
    }

    pub async fn get_cap_grants_by_access(
        &self,
        author: AgentPubKey,
        cap_access: i64,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        let mut conn = self.timed_conn().await?;
        cap_grant::get_cap_grants_by_access(&mut *conn, author, cap_access).await
    }

    pub async fn get_cap_grants_by_tag(
        &self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        let mut conn = self.timed_conn().await?;
        cap_grant::get_cap_grants_by_tag(&mut *conn, author, tag).await
    }
}
