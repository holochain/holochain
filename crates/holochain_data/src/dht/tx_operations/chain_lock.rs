//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `ChainLock` table.

use super::super::inner::chain_lock;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::ChainLockRow;
use holo_hash::AgentPubKey;
use holochain_timestamp::Timestamp;

impl TxWrite<Dht> {
    /// Try to acquire the chain lock for `author`. See the inner
    /// `chain_lock::acquire_chain_lock` for the full rule set. Returns `true`
    /// when the caller now holds the lock.
    pub async fn acquire_chain_lock(
        &mut self,
        author: &AgentPubKey,
        subject: &[u8],
        expires_at: Timestamp,
        now: Timestamp,
    ) -> sqlx::Result<bool> {
        chain_lock::acquire_chain_lock(self.conn_mut(), author, subject, expires_at, now).await
    }

    pub async fn release_chain_lock(&mut self, author: &AgentPubKey) -> sqlx::Result<()> {
        chain_lock::release_chain_lock(self.conn_mut(), author).await
    }

    pub async fn prune_expired_chain_locks(&mut self, now: Timestamp) -> sqlx::Result<()> {
        chain_lock::prune_expired_chain_locks(self.conn_mut(), now).await
    }
}

impl TxRead<Dht> {
    pub async fn get_chain_lock(
        &mut self,
        author: AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<Option<ChainLockRow>> {
        chain_lock::get_chain_lock(self.conn_mut(), author, now).await
    }
}
