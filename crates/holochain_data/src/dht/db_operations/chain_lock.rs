//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainLock` table.

use super::super::inner::chain_lock;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::ChainLockRow;
use holo_hash::AgentPubKey;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    /// Try to acquire the chain lock for `author`. See
    /// [`super::super::inner::chain_lock::acquire_chain_lock`] for the
    /// full rule set. Returns `true` when the caller now holds the lock.
    pub async fn acquire_chain_lock(
        &self,
        author: &AgentPubKey,
        subject: &[u8],
        expires_at: Timestamp,
        now: Timestamp,
    ) -> sqlx::Result<bool> {
        chain_lock::acquire_chain_lock(self.pool(), author, subject, expires_at, now).await
    }

    pub async fn release_chain_lock(&self, author: &AgentPubKey) -> sqlx::Result<()> {
        chain_lock::release_chain_lock(self.pool(), author).await
    }

    pub async fn prune_expired_chain_locks(&self, now: Timestamp) -> sqlx::Result<()> {
        chain_lock::prune_expired_chain_locks(self.pool(), now).await
    }
}

impl DbRead<Dht> {
    pub async fn get_chain_lock(
        &self,
        author: AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<Option<ChainLockRow>> {
        chain_lock::get_chain_lock(self.pool(), author, now).await
    }
}
