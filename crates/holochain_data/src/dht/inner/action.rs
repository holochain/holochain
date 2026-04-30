//! Free-standing operations against the `Action` table.

use crate::models::dht::ActionRow;
use holo_hash::{ActionHash, AgentPubKey, HoloHashed};
use holochain_integrity_types::dht_v2::{Action, ActionData, ActionHeader, RecordValidity};
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::record::SignedHashed;
use holochain_integrity_types::signature::Signature;
use holochain_zome_types::dht_v2::SignedActionHashed;
use sqlx::{Executor, Sqlite};

/// Insert an `Action` row. `record_validity` is `Some(Accepted)` for
/// self-authored actions and `None` for incoming network actions.
///
/// The stored hash is taken from `action.as_hash()` — the caller is
/// responsible for constructing the [`SignedActionHashed`] with the correct
/// hash (via [`SignedHashed::new_unchecked`] or equivalent).
pub(crate) async fn insert_action<'e, E>(
    executor: E,
    action: &SignedActionHashed,
    record_validity: Option<RecordValidity>,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    let inner: &Action = &action.hashed.content;
    let action_data_blob = holochain_serialized_bytes::encode(&inner.data)
        .map_err(|e| sqlx::Error::Protocol(format!("encode ActionData: {e}")))?;

    let entry_hash_bytes = inner.data.entry_hash().map(|h| h.get_raw_36().to_vec());
    let private_entry = match &inner.data {
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
    .bind(action.as_hash().get_raw_36())
    .bind(inner.header.author.get_raw_36())
    .bind(inner.header.action_seq as i64)
    .bind(
        inner
            .header
            .prev_action
            .as_ref()
            .map(|h| h.get_raw_36().to_vec()),
    )
    .bind(inner.header.timestamp.as_micros())
    .bind(i64::from(inner.data.action_type()))
    .bind(action_data_blob)
    .bind(action.signature().0.as_slice())
    .bind(entry_hash_bytes)
    .bind(private_entry)
    .bind(record_validity.map(i64::from))
    .execute(executor)
    .await?;
    Ok(())
}

fn row_to_signed_action_hashed(row: ActionRow) -> sqlx::Result<SignedActionHashed> {
    let data: ActionData = holochain_serialized_bytes::decode(&row.action_data).map_err(|e| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("decode ActionData: {e}"),
        )))
    })?;
    let action = Action {
        header: ActionHeader {
            author: AgentPubKey::from_raw_36(row.author),
            timestamp: holochain_timestamp::Timestamp::from_micros(row.timestamp),
            action_seq: row.seq as u32,
            prev_action: row.prev_hash.map(ActionHash::from_raw_36),
        },
        data,
    };
    let sig_bytes: [u8; 64] = row.signature.as_slice().try_into().map_err(|_| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "signature column has {} bytes, expected 64",
                row.signature.len()
            ),
        )))
    })?;
    let hashed = HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(row.hash));
    Ok(SignedHashed::with_presigned(hashed, Signature(sig_bytes)))
}

pub(crate) async fn get_action<'e, E>(
    executor: E,
    hash: ActionHash,
) -> sqlx::Result<Option<SignedActionHashed>>
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
    row.map(row_to_signed_action_hashed).transpose()
}

pub(crate) async fn get_actions_by_author<'e, E>(
    executor: E,
    author: AgentPubKey,
) -> sqlx::Result<Vec<SignedActionHashed>>
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
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}
