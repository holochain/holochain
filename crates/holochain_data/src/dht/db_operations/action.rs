//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Action` table.

use super::super::inner::action;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
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

    /// Fetch all actions with `prev_hash = prev_hash` and `hash != exclude_hash`.
    /// Used to detect chain forks during sys-validation.
    pub async fn get_actions_by_prev_hash(
        &self,
        prev_hash: &ActionHash,
        exclude_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_actions_by_prev_hash(self.pool(), prev_hash, exclude_hash).await
    }

    /// The entry's `StoreEntry` create actions at `validation_status`.
    pub async fn get_entry_creates(
        &self,
        entry_hash: &EntryHash,
        author: Option<&AgentPubKey>,
        validation_status: i64,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_entry_creates(self.pool(), entry_hash, author, validation_status).await
    }

    /// The `Delete` actions on `entry_hash`.
    pub async fn get_delete_actions_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_delete_actions_for_entry(self.pool(), entry_hash).await
    }

    /// The `Update` actions from `entry_hash`.
    pub async fn get_update_actions_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_update_actions_for_entry(self.pool(), entry_hash).await
    }

    /// Live `StoreEntry` create actions for `entry_hash` (valid, integrated,
    /// not deleted, visible to `author`), ordered by integration time.
    pub async fn get_live_entry_creates(
        &self,
        entry_hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_live_entry_creates(self.pool(), entry_hash, author).await
    }

    /// The `Delete` actions targeting `record_action_hash`.
    pub async fn get_delete_actions_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_delete_actions_for_record(self.pool(), record_action_hash).await
    }

    /// The `Update` actions that update `record_action_hash`.
    pub async fn get_update_actions_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_update_actions_for_record(self.pool(), record_action_hash).await
    }
}
