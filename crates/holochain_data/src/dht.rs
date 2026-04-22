//! DHT database operations.
//!
//! Free-standing `async fn`s over `Executor` / `Acquire`, mirrored onto
//! the `Dht` database handles (`DbRead` / `DbWrite` / `TxRead` / `TxWrite`).

use crate::handles::{DbRead, DbWrite, TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::*;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_integrity_types::dht_v2::{Action, ActionData, ActionHeader, RecordValidity};
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::signature::Signature;
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
}
