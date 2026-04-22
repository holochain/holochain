//! DHT database operations.
//!
//! Free-standing `async fn`s over `Executor` / `Acquire`, mirrored onto
//! the `Dht` database handles (`DbRead` / `DbWrite` / `TxRead` / `TxWrite`).

use crate::handles::{DbRead, DbWrite, TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::*;
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_integrity_types::dht_v2::{Action, ActionData, ActionHeader, RecordValidity};
use holochain_integrity_types::entry::Entry;
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::signature::Signature;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

// ============================================================================
// Action operations
// ============================================================================

/// Insert an `Action` row. `record_validity` is `Some(Accepted)` for
/// self-authored actions and `None` for incoming network actions.
async fn insert_action_impl<'e, E>(
    executor: E,
    action: &Action,
    signature: &Signature,
    record_validity: Option<RecordValidity>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let action_data_blob = holochain_serialized_bytes::encode(&action.data)
        .map_err(|e| sqlx::Error::Protocol(format!("encode ActionData: {e}")))?;

    let entry_hash_bytes = action.data.entry_hash().map(|h| h.get_raw_36().to_vec());
    let private_entry = match &action.data {
        ActionData::Create(d) => Some(*d.entry_type.visibility() == EntryVisibility::Private),
        ActionData::Update(d) => Some(*d.entry_type.visibility() == EntryVisibility::Private),
        _ => None,
    }
    .map(|b| b as i64);

    sqlx::query(
        "INSERT INTO Action (hash, author, seq, prev_hash, timestamp, action_type,
                             action_data, signature, entry_hash, private_entry,
                             record_validity)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(action.hash.get_raw_36())
    .bind(action.header.author.get_raw_36())
    .bind(action.header.action_seq as i64)
    .bind(
        action
            .header
            .prev_action
            .as_ref()
            .map(|h| h.get_raw_36().to_vec()),
    )
    .bind(action.header.timestamp.as_micros())
    .bind(i64::from(action.data.action_type()))
    .bind(action_data_blob)
    .bind(signature.0.as_slice())
    .bind(entry_hash_bytes)
    .bind(private_entry)
    .bind(record_validity.map(i64::from))
    .execute(executor)
    .await?;
    Ok(())
}

fn row_to_action(row: ActionRow) -> sqlx::Result<Action> {
    let data: ActionData = holochain_serialized_bytes::decode(&row.action_data).map_err(|e| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("decode ActionData: {e}"),
        )))
    })?;
    Ok(Action {
        hash: ActionHash::from_raw_36(row.hash),
        header: ActionHeader {
            author: AgentPubKey::from_raw_36(row.author),
            timestamp: holochain_timestamp::Timestamp::from_micros(row.timestamp),
            action_seq: row.seq as u32,
            prev_action: row.prev_hash.map(ActionHash::from_raw_36),
        },
        data,
    })
}

async fn get_action_impl<'e, E>(executor: E, hash: ActionHash) -> sqlx::Result<Option<Action>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<ActionRow> = sqlx::query_as(
        "SELECT hash, author, seq, prev_hash, timestamp, action_type,
                action_data, signature, entry_hash, private_entry, record_validity
         FROM Action WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await?;
    row.map(row_to_action).transpose()
}

async fn get_actions_by_author_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
) -> sqlx::Result<Vec<Action>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT hash, author, seq, prev_hash, timestamp, action_type,
                action_data, signature, entry_hash, private_entry, record_validity
         FROM Action WHERE author = ? ORDER BY seq ASC",
    )
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_action).collect()
}

// ============================================================================
// DbRead / DbWrite / TxRead / TxWrite wrappers
// ============================================================================

impl DbWrite<Dht> {
    /// Insert an `Action` row.
    pub async fn insert_action(
        &self,
        action: &Action,
        signature: &Signature,
        record_validity: Option<RecordValidity>,
    ) -> sqlx::Result<()> {
        insert_action_impl(self.pool(), action, signature, record_validity).await
    }
}

impl DbRead<Dht> {
    /// Fetch a single `Action` by hash.
    pub async fn get_action(&self, hash: ActionHash) -> sqlx::Result<Option<Action>> {
        get_action_impl(self.pool(), hash).await
    }

    /// Fetch all actions for a given author, ordered by `action_seq` ascending.
    pub async fn get_actions_by_author(
        &self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<Action>> {
        get_actions_by_author_impl(self.pool(), author).await
    }
}

impl TxWrite<Dht> {
    /// Insert an `Action` row.
    pub async fn insert_action(
        &mut self,
        action: &Action,
        signature: &Signature,
        record_validity: Option<RecordValidity>,
    ) -> sqlx::Result<()> {
        insert_action_impl(self.conn_mut(), action, signature, record_validity).await
    }
}

impl TxRead<Dht> {
    /// Fetch a single `Action` by hash.
    pub async fn get_action(&mut self, hash: ActionHash) -> sqlx::Result<Option<Action>> {
        get_action_impl(self.conn_mut(), hash).await
    }

    /// Fetch all actions for a given author, ordered by `action_seq` ascending.
    pub async fn get_actions_by_author(
        &mut self,
        author: AgentPubKey,
    ) -> sqlx::Result<Vec<Action>> {
        get_actions_by_author_impl(self.conn_mut(), author).await
    }
}

// ============================================================================
// Entry / PrivateEntry operations
// ============================================================================

async fn insert_entry_impl<'e, E>(
    executor: E,
    hash: &EntryHash,
    entry: &Entry,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let blob = holochain_serialized_bytes::encode(entry)
        .map_err(|e| sqlx::Error::Protocol(format!("encode Entry: {e}")))?;
    sqlx::query("INSERT INTO Entry (hash, blob) VALUES (?, ?)")
        .bind(hash.get_raw_36())
        .bind(blob)
        .execute(executor)
        .await?;
    Ok(())
}

async fn get_entry_impl<'e, E>(
    executor: E,
    hash: EntryHash,
) -> sqlx::Result<Option<Entry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<EntryRow> = sqlx::query_as(
        "SELECT hash, blob FROM Entry WHERE hash = ?",
    )
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await?;
    row.map(|r| {
        holochain_serialized_bytes::decode::<_, Entry>(&r.blob).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("decode Entry: {e}"),
            )))
        })
    })
    .transpose()
}

async fn insert_private_entry_impl<'e, E>(
    executor: E,
    hash: &EntryHash,
    author: &AgentPubKey,
    entry: &Entry,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let blob = holochain_serialized_bytes::encode(entry)
        .map_err(|e| sqlx::Error::Protocol(format!("encode Entry: {e}")))?;
    sqlx::query("INSERT INTO PrivateEntry (hash, author, blob) VALUES (?, ?, ?)")
        .bind(hash.get_raw_36())
        .bind(author.get_raw_36())
        .bind(blob)
        .execute(executor)
        .await?;
    Ok(())
}

async fn get_private_entry_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    hash: EntryHash,
) -> sqlx::Result<Option<Entry>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<PrivateEntryRow> = sqlx::query_as(
        "SELECT hash, author, blob FROM PrivateEntry WHERE author = ? AND hash = ?",
    )
    .bind(author.get_raw_36())
    .bind(hash.get_raw_36())
    .fetch_optional(executor)
    .await?;
    row.map(|r| {
        holochain_serialized_bytes::decode::<_, Entry>(&r.blob).map_err(|e| {
            sqlx::Error::Decode(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("decode Entry: {e}"),
            )))
        })
    })
    .transpose()
}

impl DbWrite<Dht> {
    pub async fn insert_entry(&self, hash: &EntryHash, entry: &Entry) -> sqlx::Result<()> {
        insert_entry_impl(self.pool(), hash, entry).await
    }

    pub async fn insert_private_entry(
        &self,
        hash: &EntryHash,
        author: &AgentPubKey,
        entry: &Entry,
    ) -> sqlx::Result<()> {
        insert_private_entry_impl(self.pool(), hash, author, entry).await
    }
}

impl DbRead<Dht> {
    pub async fn get_entry(&self, hash: EntryHash) -> sqlx::Result<Option<Entry>> {
        get_entry_impl(self.pool(), hash).await
    }

    pub async fn get_private_entry(
        &self,
        author: AgentPubKey,
        hash: EntryHash,
    ) -> sqlx::Result<Option<Entry>> {
        get_private_entry_impl(self.pool(), author, hash).await
    }
}

impl TxWrite<Dht> {
    pub async fn insert_entry(&mut self, hash: &EntryHash, entry: &Entry) -> sqlx::Result<()> {
        insert_entry_impl(self.conn_mut(), hash, entry).await
    }

    pub async fn insert_private_entry(
        &mut self,
        hash: &EntryHash,
        author: &AgentPubKey,
        entry: &Entry,
    ) -> sqlx::Result<()> {
        insert_private_entry_impl(self.conn_mut(), hash, author, entry).await
    }
}

impl TxRead<Dht> {
    pub async fn get_entry(&mut self, hash: EntryHash) -> sqlx::Result<Option<Entry>> {
        get_entry_impl(self.conn_mut(), hash).await
    }

    pub async fn get_private_entry(
        &mut self,
        author: AgentPubKey,
        hash: EntryHash,
    ) -> sqlx::Result<Option<Entry>> {
        get_private_entry_impl(self.conn_mut(), author, hash).await
    }
}

// ============================================================================
// CapGrant / CapClaim operations
// ============================================================================

async fn insert_cap_grant_impl<'e, E>(
    executor: E,
    action_hash: &ActionHash,
    cap_access: i64,
    tag: Option<&str>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO CapGrant (action_hash, cap_access, tag) VALUES (?, ?, ?)")
        .bind(action_hash.get_raw_36())
        .bind(cap_access)
        .bind(tag)
        .execute(executor)
        .await?;
    Ok(())
}

async fn get_cap_grants_by_access_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    cap_access: i64,
) -> sqlx::Result<Vec<CapGrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT cg.action_hash, cg.cap_access, cg.tag
         FROM CapGrant cg
         JOIN Action ON cg.action_hash = Action.hash
         WHERE cg.cap_access = ? AND Action.author = ?
         ORDER BY Action.seq",
    )
    .bind(cap_access)
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}

async fn get_cap_grants_by_tag_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    tag: &str,
) -> sqlx::Result<Vec<CapGrantRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT cg.action_hash, cg.cap_access, cg.tag
         FROM CapGrant cg
         JOIN Action ON cg.action_hash = Action.hash
         WHERE cg.tag = ? AND Action.author = ?
         ORDER BY Action.seq",
    )
    .bind(tag)
    .bind(author.get_raw_36())
    .fetch_all(executor)
    .await
}

async fn insert_cap_claim_impl<'e, E>(
    executor: E,
    author: &AgentPubKey,
    tag: &str,
    grantor: &AgentPubKey,
    secret: &[u8],
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("INSERT INTO CapClaim (author, tag, grantor, secret) VALUES (?, ?, ?, ?)")
        .bind(author.get_raw_36())
        .bind(tag)
        .bind(grantor.get_raw_36())
        .bind(secret)
        .execute(executor)
        .await?;
    Ok(())
}

async fn get_cap_claims_by_grantor_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    grantor: AgentPubKey,
) -> sqlx::Result<Vec<CapClaimRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT id, author, tag, grantor, secret FROM CapClaim
         WHERE author = ? AND grantor = ? ORDER BY id",
    )
    .bind(author.get_raw_36())
    .bind(grantor.get_raw_36())
    .fetch_all(executor)
    .await
}

async fn get_cap_claims_by_tag_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    tag: &str,
) -> sqlx::Result<Vec<CapClaimRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT id, author, tag, grantor, secret FROM CapClaim
         WHERE author = ? AND tag = ? ORDER BY id",
    )
    .bind(author.get_raw_36())
    .bind(tag)
    .fetch_all(executor)
    .await
}

impl DbWrite<Dht> {
    pub async fn insert_cap_grant(
        &self,
        action_hash: &ActionHash,
        cap_access: i64,
        tag: Option<&str>,
    ) -> sqlx::Result<()> {
        insert_cap_grant_impl(self.pool(), action_hash, cap_access, tag).await
    }

    pub async fn insert_cap_claim(
        &self,
        author: &AgentPubKey,
        tag: &str,
        grantor: &AgentPubKey,
        secret: &[u8],
    ) -> sqlx::Result<()> {
        insert_cap_claim_impl(self.pool(), author, tag, grantor, secret).await
    }
}

impl DbRead<Dht> {
    pub async fn get_cap_grants_by_access(
        &self,
        author: AgentPubKey,
        cap_access: i64,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        get_cap_grants_by_access_impl(self.pool(), author, cap_access).await
    }

    pub async fn get_cap_grants_by_tag(
        &self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        get_cap_grants_by_tag_impl(self.pool(), author, tag).await
    }

    pub async fn get_cap_claims_by_grantor(
        &self,
        author: AgentPubKey,
        grantor: AgentPubKey,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        get_cap_claims_by_grantor_impl(self.pool(), author, grantor).await
    }

    pub async fn get_cap_claims_by_tag(
        &self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        get_cap_claims_by_tag_impl(self.pool(), author, tag).await
    }
}

impl TxWrite<Dht> {
    pub async fn insert_cap_grant(
        &mut self,
        action_hash: &ActionHash,
        cap_access: i64,
        tag: Option<&str>,
    ) -> sqlx::Result<()> {
        insert_cap_grant_impl(self.conn_mut(), action_hash, cap_access, tag).await
    }

    pub async fn insert_cap_claim(
        &mut self,
        author: &AgentPubKey,
        tag: &str,
        grantor: &AgentPubKey,
        secret: &[u8],
    ) -> sqlx::Result<()> {
        insert_cap_claim_impl(self.conn_mut(), author, tag, grantor, secret).await
    }
}

impl TxRead<Dht> {
    pub async fn get_cap_grants_by_access(
        &mut self,
        author: AgentPubKey,
        cap_access: i64,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        get_cap_grants_by_access_impl(self.conn_mut(), author, cap_access).await
    }

    pub async fn get_cap_grants_by_tag(
        &mut self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapGrantRow>> {
        get_cap_grants_by_tag_impl(self.conn_mut(), author, tag).await
    }

    pub async fn get_cap_claims_by_grantor(
        &mut self,
        author: AgentPubKey,
        grantor: AgentPubKey,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        get_cap_claims_by_grantor_impl(self.conn_mut(), author, grantor).await
    }

    pub async fn get_cap_claims_by_tag(
        &mut self,
        author: AgentPubKey,
        tag: &str,
    ) -> sqlx::Result<Vec<CapClaimRow>> {
        get_cap_claims_by_tag_impl(self.conn_mut(), author, tag).await
    }
}

// ============================================================================
// ChainLock operations
// ============================================================================

async fn acquire_chain_lock_impl<'e, E>(
    executor: E,
    author: &AgentPubKey,
    subject: &[u8],
    expires_at: Timestamp,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ChainLock (author, subject, expires_at_timestamp)
         VALUES (?, ?, ?)
         ON CONFLICT(author) DO UPDATE SET
            subject = excluded.subject,
            expires_at_timestamp = excluded.expires_at_timestamp",
    )
    .bind(author.get_raw_36())
    .bind(subject)
    .bind(expires_at.as_micros())
    .execute(executor)
    .await?;
    Ok(())
}

async fn get_chain_lock_impl<'e, E>(
    executor: E,
    author: AgentPubKey,
    now: Timestamp,
) -> sqlx::Result<Option<ChainLockRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT author, subject, expires_at_timestamp FROM ChainLock
         WHERE author = ? AND expires_at_timestamp > ?",
    )
    .bind(author.get_raw_36())
    .bind(now.as_micros())
    .fetch_optional(executor)
    .await
}

async fn release_chain_lock_impl<'e, E>(
    executor: E,
    author: &AgentPubKey,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM ChainLock WHERE author = ?")
        .bind(author.get_raw_36())
        .execute(executor)
        .await?;
    Ok(())
}

async fn prune_expired_chain_locks_impl<'e, E>(
    executor: E,
    now: Timestamp,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM ChainLock WHERE expires_at_timestamp <= ?")
        .bind(now.as_micros())
        .execute(executor)
        .await?;
    Ok(())
}

impl DbWrite<Dht> {
    pub async fn acquire_chain_lock(
        &self,
        author: &AgentPubKey,
        subject: &[u8],
        expires_at: Timestamp,
    ) -> sqlx::Result<()> {
        acquire_chain_lock_impl(self.pool(), author, subject, expires_at).await
    }

    pub async fn release_chain_lock(&self, author: &AgentPubKey) -> sqlx::Result<()> {
        release_chain_lock_impl(self.pool(), author).await
    }

    pub async fn prune_expired_chain_locks(&self, now: Timestamp) -> sqlx::Result<()> {
        prune_expired_chain_locks_impl(self.pool(), now).await
    }
}

impl DbRead<Dht> {
    pub async fn get_chain_lock(
        &self,
        author: AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<Option<ChainLockRow>> {
        get_chain_lock_impl(self.pool(), author, now).await
    }
}

impl TxWrite<Dht> {
    pub async fn acquire_chain_lock(
        &mut self,
        author: &AgentPubKey,
        subject: &[u8],
        expires_at: Timestamp,
    ) -> sqlx::Result<()> {
        acquire_chain_lock_impl(self.conn_mut(), author, subject, expires_at).await
    }

    pub async fn release_chain_lock(&mut self, author: &AgentPubKey) -> sqlx::Result<()> {
        release_chain_lock_impl(self.conn_mut(), author).await
    }

    pub async fn prune_expired_chain_locks(&mut self, now: Timestamp) -> sqlx::Result<()> {
        prune_expired_chain_locks_impl(self.conn_mut(), now).await
    }
}

impl TxRead<Dht> {
    pub async fn get_chain_lock(
        &mut self,
        author: AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<Option<ChainLockRow>> {
        get_chain_lock_impl(self.conn_mut(), author, now).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::{ActionHash, AgentPubKey, DnaHash};
    use holochain_integrity_types::dht_v2::{
        ActionData, ActionHeader, DnaData, InitZomesCompleteData,
    };
    use holochain_timestamp::Timestamp;
    use std::sync::Arc;

    fn dht_db_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn sample_action(seed: u8) -> (Action, Signature) {
        let action = Action {
            hash: ActionHash::from_raw_36(vec![seed; 36]),
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: Timestamp::from_micros(1_000_000 + seed as i64),
                action_seq: seed as u32,
                prev_action: if seed == 0 {
                    None
                } else {
                    Some(ActionHash::from_raw_36(vec![seed - 1; 36]))
                },
            },
            data: if seed == 0 {
                ActionData::Dna(DnaData {
                    dna_hash: DnaHash::from_raw_36(vec![0u8; 36]),
                })
            } else {
                ActionData::InitZomesComplete(InitZomesCompleteData {})
            },
        };
        let signature = Signature([seed; 64]);
        (action, signature)
    }

    #[tokio::test]
    async fn action_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (action, signature) = sample_action(0);

        db.insert_action(&action, &signature, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let fetched = db
            .as_ref()
            .get_action(action.hash.clone())
            .await
            .unwrap()
            .expect("action not found");

        assert_eq!(fetched, action);
    }

    #[tokio::test]
    async fn actions_by_author() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        for seed in 0..3u8 {
            let (action, signature) = sample_action(seed);
            db.insert_action(&action, &signature, Some(RecordValidity::Accepted))
                .await
                .unwrap();
        }

        let author = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let actions = db.as_ref().get_actions_by_author(author).await.unwrap();
        assert_eq!(actions.len(), 3);
        // Ordered by seq ascending.
        for (i, action) in actions.iter().enumerate() {
            assert_eq!(action.header.action_seq, i as u32);
        }
    }

    use holo_hash::EntryHash;

    fn sample_entry(seed: u8) -> (EntryHash, Entry) {
        let entry = Entry::App(
            holochain_integrity_types::entry::AppEntryBytes(
                holochain_serialized_bytes::UnsafeBytes::from(vec![seed; 16]).into(),
            ),
        );
        let hash = EntryHash::from_raw_36(vec![seed; 36]);
        (hash, entry)
    }

    #[tokio::test]
    async fn entry_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(7);
        db.insert_entry(&hash, &entry).await.unwrap();
        let fetched = db.as_ref().get_entry(hash.clone()).await.unwrap();
        assert_eq!(fetched, Some(entry));
    }

    #[tokio::test]
    async fn private_entry_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(11);
        let author = AgentPubKey::from_raw_36(vec![2u8; 36]);
        db.insert_private_entry(&hash, &author, &entry).await.unwrap();
        let fetched = db
            .as_ref()
            .get_private_entry(author.clone(), hash.clone())
            .await
            .unwrap();
        assert_eq!(fetched, Some(entry));
    }

    #[tokio::test]
    async fn private_entry_isolated_from_entry() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (hash, entry) = sample_entry(13);
        let author = AgentPubKey::from_raw_36(vec![3u8; 36]);
        db.insert_private_entry(&hash, &author, &entry).await.unwrap();
        // Not visible via the public Entry read.
        assert_eq!(db.as_ref().get_entry(hash.clone()).await.unwrap(), None);
    }

    /// Verifies that a TxWrite bundling an Action + Entry insert can be rolled back
    /// and neither survives. Also exercises the Tx* wrapper methods.
    #[tokio::test]
    async fn tx_action_and_entry_rollback_discards() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let (action, signature) = sample_action(0);
        let (entry_hash, entry) = sample_entry(42);

        let mut tx = db.begin().await.unwrap();
        tx.insert_action(&action, &signature, Some(RecordValidity::Accepted))
            .await
            .unwrap();
        tx.insert_entry(&entry_hash, &entry).await.unwrap();
        tx.rollback().await.unwrap();

        assert!(db
            .as_ref()
            .get_action(action.hash)
            .await
            .unwrap()
            .is_none());
        assert!(db
            .as_ref()
            .get_entry(entry_hash)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn cap_grant_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        // Seed the parent Action (FK).
        let (action, signature) = sample_action(0);
        db.insert_action(&action, &signature, Some(RecordValidity::Accepted))
            .await
            .unwrap();

        let author = action.header.author.clone();
        db.insert_cap_grant(&action.hash, 1 /* Transferable */, Some("my-tag"))
            .await
            .unwrap();

        let by_access = db
            .as_ref()
            .get_cap_grants_by_access(author.clone(), 1)
            .await
            .unwrap();
        assert_eq!(by_access.len(), 1);
        assert_eq!(by_access[0].action_hash, action.hash.get_raw_36().to_vec());

        let by_tag = db
            .as_ref()
            .get_cap_grants_by_tag(author, "my-tag")
            .await
            .unwrap();
        assert_eq!(by_tag.len(), 1);
    }

    #[tokio::test]
    async fn cap_claim_roundtrip() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![5u8; 36]);
        let grantor = AgentPubKey::from_raw_36(vec![6u8; 36]);

        db.insert_cap_claim(&author, "claim-tag", &grantor, &[9u8; 32])
            .await
            .unwrap();

        let by_grantor = db
            .as_ref()
            .get_cap_claims_by_grantor(author.clone(), grantor)
            .await
            .unwrap();
        assert_eq!(by_grantor.len(), 1);
        assert_eq!(by_grantor[0].tag, "claim-tag");

        let by_tag = db
            .as_ref()
            .get_cap_claims_by_tag(author, "claim-tag")
            .await
            .unwrap();
        assert_eq!(by_tag.len(), 1);
    }

    #[tokio::test]
    async fn cap_grant_requires_action_fk() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let missing = ActionHash::from_raw_36(vec![42u8; 36]);
        let err = db
            .insert_cap_grant(&missing, 0, None)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.to_lowercase().contains("foreign key"), "got: {err}");
    }

    #[tokio::test]
    async fn chain_lock_acquire_and_read() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        let subject = vec![1u8; 32];

        db.acquire_chain_lock(&author, &subject, Timestamp::from_micros(10_000))
            .await
            .unwrap();

        let lock = db
            .as_ref()
            .get_chain_lock(author.clone(), Timestamp::from_micros(5_000))
            .await
            .unwrap()
            .expect("expected active lock");
        assert_eq!(lock.subject, subject);
    }

    #[tokio::test]
    async fn chain_lock_upsert_replaces_subject() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        db.acquire_chain_lock(&author, &[1u8; 32], Timestamp::from_micros(10_000))
            .await
            .unwrap();
        db.acquire_chain_lock(&author, &[2u8; 32], Timestamp::from_micros(20_000))
            .await
            .unwrap();

        let lock = db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(5_000))
            .await
            .unwrap()
            .expect("expected lock");
        assert_eq!(lock.subject, vec![2u8; 32]);
        assert_eq!(lock.expires_at_timestamp, 20_000);
    }

    #[tokio::test]
    async fn chain_lock_release_and_prune() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let a = AgentPubKey::from_raw_36(vec![1u8; 36]);
        let b = AgentPubKey::from_raw_36(vec![2u8; 36]);

        db.acquire_chain_lock(&a, &[1u8; 32], Timestamp::from_micros(100))
            .await
            .unwrap();
        db.acquire_chain_lock(&b, &[2u8; 32], Timestamp::from_micros(1_000))
            .await
            .unwrap();

        db.release_chain_lock(&a).await.unwrap();
        assert!(db
            .as_ref()
            .get_chain_lock(a.clone(), Timestamp::from_micros(50))
            .await
            .unwrap()
            .is_none());

        // Prune anything expired at t=500; b's lock (expires 1000) should survive.
        db.prune_expired_chain_locks(Timestamp::from_micros(500))
            .await
            .unwrap();
        assert!(db
            .as_ref()
            .get_chain_lock(b, Timestamp::from_micros(200))
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn chain_lock_expired_is_not_returned() {
        let db = test_open_db(dht_db_id()).await.unwrap();
        let author = AgentPubKey::from_raw_36(vec![9u8; 36]);
        db.acquire_chain_lock(&author, &[3u8; 32], Timestamp::from_micros(100))
            .await
            .unwrap();
        // Now is past expiry.
        assert!(db
            .as_ref()
            .get_chain_lock(author, Timestamp::from_micros(200))
            .await
            .unwrap()
            .is_none());
    }
}
