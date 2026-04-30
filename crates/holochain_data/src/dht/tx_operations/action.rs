//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `Action` table.

use super::super::inner::action;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_integrity_types::dht_v2::RecordValidity;
use holochain_zome_types::dht_v2::SignedActionHashed;

impl TxWrite<Dht> {
    /// Insert an `Action` row, storing its signature and pre-computed hash.
    pub async fn insert_action(
        &mut self,
        action: &SignedActionHashed,
        record_validity: Option<RecordValidity>,
    ) -> sqlx::Result<()> {
        action::insert_action(self.conn_mut(), action, record_validity).await
    }
}

impl TxRead<Dht> {
    /// Fetch a single action by hash, returning it with its stored signature
    /// and hash as a [`SignedActionHashed`].
    pub async fn get_action(
        &mut self,
        hash: ActionHash,
    ) -> sqlx::Result<Option<SignedActionHashed>> {
        action::get_action(self.conn_mut(), hash).await
    }

    /// Fetch all actions for a given author, ordered by `action_seq` ascending.
    pub async fn get_actions_by_author(
        &mut self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        action::get_actions_by_author(self.conn_mut(), author).await
    }
}
