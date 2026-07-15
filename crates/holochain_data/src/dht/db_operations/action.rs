//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Action` table.

use super::super::inner::action;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::AgentActivityItem;
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, EntryHash};
use holochain_integrity_types::action::RecordValidity;
use holochain_zome_types::action::SignedActionHashed;

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
        let mut conn = self.timed_conn().await?;
        action::get_action(&mut *conn, hash).await
    }

    /// Fetch all actions for a given author, ordered by `action_seq` ascending.
    pub async fn get_actions_by_author(
        &self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_actions_by_author(&mut *conn, author).await
    }

    /// Count actions authored by `author`, capped at `cap`. See
    /// `action::count_author_actions_capped`.
    pub async fn count_author_actions_capped(
        &self,
        author: &AgentPubKey,
        cap: i64,
    ) -> sqlx::Result<i64> {
        let mut conn = self.timed_conn().await?;
        action::count_author_actions_capped(&mut *conn, author, cap).await
    }

    /// The author's chain head: the highest-sequence action they authored, or
    /// `None` for an empty chain (pre-genesis).
    pub async fn chain_head_for_author(
        &self,
        author: &AgentPubKey,
    ) -> sqlx::Result<Option<(ActionHash, u32, holochain_timestamp::Timestamp)>> {
        let mut conn = self.timed_conn().await?;
        action::chain_head_for_author(&mut *conn, author).await
    }

    /// Integrated `AgentActivity` actions for `author`, ordered by
    /// chain sequence. `include_entries` joins the public entry (Full mode).
    pub async fn get_agent_activity(
        &self,
        author: AgentPubKey,
        include_entries: bool,
    ) -> sqlx::Result<Vec<AgentActivityItem>> {
        let mut conn = self.timed_conn().await?;
        action::get_agent_activity(&mut *conn, &author, include_entries).await
    }

    /// Bounded `AgentActivity` scan: `author`'s integrated actions with
    /// `seq <= chain_top_seq` and (optionally) `seq >= until_seq`, ordered by
    /// `seq DESC, hash DESC`.
    pub async fn get_filtered_agent_activity(
        &self,
        author: AgentPubKey,
        chain_top_seq: u32,
        until_seq: Option<u32>,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_filtered_agent_activity(&mut *conn, &author, chain_top_seq, until_seq).await
    }

    /// The chain sequence and authored timestamp of `action_hash`, if it is an
    /// integrated `AgentActivity` action authored by `author`.
    pub async fn get_action_seq_and_timestamp(
        &self,
        author: AgentPubKey,
        action_hash: ActionHash,
    ) -> sqlx::Result<Option<(u32, holochain_timestamp::Timestamp)>> {
        let mut conn = self.timed_conn().await?;
        action::get_action_seq_and_timestamp(&mut *conn, &author, &action_hash).await
    }

    /// Fetch all actions with `prev_hash = prev_hash` and `hash != exclude_hash`.
    /// Used to detect chain forks during sys-validation.
    pub async fn get_actions_by_prev_hash(
        &self,
        prev_hash: &ActionHash,
        exclude_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_actions_by_prev_hash(&mut *conn, prev_hash, exclude_hash).await
    }

    /// The entry's `CreateEntry` create actions at `validation_status`.
    pub async fn get_create_actions_for_entry(
        &self,
        entry_hash: &EntryHash,
        author: Option<&AgentPubKey>,
        validation_status: RecordValidity,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_create_actions_for_entry(&mut *conn, entry_hash, author, validation_status)
            .await
    }

    /// The `Delete` actions on `entry_hash`.
    pub async fn get_delete_actions_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_delete_actions_for_entry(&mut *conn, entry_hash).await
    }

    /// The `Update` actions from `entry_hash`.
    pub async fn get_update_actions_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_update_actions_for_entry(&mut *conn, entry_hash).await
    }

    /// Live `CreateEntry` create actions for `entry_hash` (valid, integrated,
    /// not deleted, visible to `author`), ordered by integration time.
    pub async fn get_live_entry_creates(
        &self,
        entry_hash: &EntryHash,
        author: Option<&AgentPubKey>,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_live_entry_creates(&mut *conn, entry_hash, author).await
    }

    /// The `Delete` actions targeting `record_action_hash`.
    pub async fn get_delete_actions_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_delete_actions_for_record(&mut *conn, record_action_hash).await
    }

    /// The `Update` actions that update `record_action_hash`.
    pub async fn get_update_actions_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_update_actions_for_record(&mut *conn, record_action_hash).await
    }

    /// The live `CreateLink` actions on `base` (excluding tombstoned links).
    pub async fn get_live_link_actions(
        &self,
        base: &AnyLinkableHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_live_link_actions(&mut *conn, base).await
    }

    /// All `CreateLink` actions on `base` (live and tombstoned).
    pub async fn get_link_create_actions(
        &self,
        base: &AnyLinkableHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_link_create_actions(&mut *conn, base).await
    }

    /// The `DeleteLink` actions tombstoning `create_link_hash`.
    pub async fn get_delete_link_actions(
        &self,
        create_link_hash: &ActionHash,
    ) -> sqlx::Result<Vec<SignedActionHashed>> {
        let mut conn = self.timed_conn().await?;
        action::get_delete_link_actions(&mut *conn, create_link_hash).await
    }

    /// Authority-serving create-link actions for `base` (locally-validated only),
    /// each with its validation status.
    pub async fn get_authority_link_creates(
        &self,
        base: &AnyLinkableHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_link_creates(&mut *conn, base).await
    }

    /// Authority-serving delete-link actions targeting `base`'s links
    /// (locally-validated only), each with its validation status.
    pub async fn get_authority_delete_links(
        &self,
        base: &AnyLinkableHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_delete_links(&mut *conn, base).await
    }

    /// Authority-serving `CreateRecord` action for `action_hash` (locally-validated
    /// only), with its validation status.
    pub async fn get_authority_store_record(
        &self,
        action_hash: &ActionHash,
    ) -> sqlx::Result<Option<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_store_record(&mut *conn, action_hash).await
    }

    /// Authority-serving deletes targeting record `record_action_hash`
    /// (locally-validated only), each with its validation status.
    pub async fn get_authority_deletes_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_deletes_for_record(&mut *conn, record_action_hash).await
    }

    /// Authority-serving updates targeting record `record_action_hash`
    /// (locally-validated only), each with its validation status.
    pub async fn get_authority_updates_for_record(
        &self,
        record_action_hash: &ActionHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_updates_for_record(&mut *conn, record_action_hash).await
    }

    /// Authority-serving create actions for entry `entry_hash` (locally-validated
    /// only), each with its validation status.
    pub async fn get_authority_entry_creates(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_entry_creates(&mut *conn, entry_hash).await
    }

    /// Authority-serving deletes targeting entry `entry_hash` (locally-validated
    /// only), each with its validation status.
    pub async fn get_authority_deletes_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_deletes_for_entry(&mut *conn, entry_hash).await
    }

    /// Authority-serving updates targeting entry `entry_hash` (locally-validated
    /// only), each with its validation status.
    pub async fn get_authority_updates_for_entry(
        &self,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>> {
        let mut conn = self.timed_conn().await?;
        action::get_authority_updates_for_entry(&mut *conn, entry_hash).await
    }
}
