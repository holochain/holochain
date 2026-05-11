//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Action` table.

use super::super::inner::action;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_integrity_types::dht_v2::RecordValidity;
use holochain_zome_types::dht_v2::SignedActionHashed;

impl DbWrite<Dht> {
    /// Insert an `Action` row, storing its signature and pre-computed hash.
    pub async fn insert_action(
        &self,
        action: &SignedActionHashed,
        record_validity: Option<RecordValidity>,
    ) -> sqlx::Result<()> {
        action::insert_action(self.pool(), action, record_validity).await
    }
}

impl DbRead<Dht> {
    /// Fetch a single action by hash, returning it with its stored signature
    /// and hash as a [`SignedActionHashed`].
    pub async fn get_action(&self, hash: ActionHash) -> sqlx::Result<Option<SignedActionHashed>> {
        action::get_action(self.pool(), hash).await
    }

    /// Fetch all actions for a given author, ordered by `action_seq` ascending.
    pub async fn get_actions_by_author(
        &self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_actions_by_author(self.pool(), author).await
    }
}
