//! Free-standing operations against the `Action` table.

use crate::models::dht::ActionRow;
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, EntryHash, HoloHashed};
use holochain_integrity_types::dht_v2::{Action, ActionData, ActionHeader, RecordValidity};
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::record::SignedHashed;
use holochain_integrity_types::signature::Signature;
use holochain_zome_types::dht_v2::SignedActionHashed;
use holochain_zome_types::op::ChainOpType;
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

/// Return the live `StoreEntry` create actions for `entry_hash`: valid,
/// integrated `StoreEntry` ops on that basis whose action has no `DeletedRecord`,
/// and whose entry is visible to `author` (public, or private and authored by
/// `author`). Ordered by integration time.
pub(crate) async fn get_live_entry_creates<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
    author: Option<&AgentPubKey>,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         WHERE c.basis_hash = ?
           AND c.op_type = ?
           AND c.validation_status = ?
           AND c.when_integrated IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM DeletedRecord d WHERE d.deletes_action_hash = a.hash)
           AND (a.private_entry = 0 OR a.private_entry IS NULL OR a.author = ?)
         ORDER BY c.when_integrated",
    )
    .bind(entry_hash.get_raw_36())
    .bind(i64::from(ChainOpType::StoreEntry))
    .bind(i64::from(RecordValidity::Accepted))
    .bind(author.map(|a| a.get_raw_36().to_vec()))
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The entry's `StoreEntry` create actions at validation status
/// `validation_status` (integrated, visible to `author`). Unlike
/// `get_live_entry_creates`, this does NOT exclude deleted creates.
pub(crate) async fn get_entry_creates<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
    author: Option<&AgentPubKey>,
    validation_status: i64,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         WHERE c.basis_hash = ?
           AND c.op_type = ?
           AND c.validation_status = ?
           AND c.when_integrated IS NOT NULL
           AND (a.private_entry = 0 OR a.private_entry IS NULL OR a.author = ?)
         ORDER BY c.when_integrated",
    )
    .bind(entry_hash.get_raw_36())
    .bind(i64::from(ChainOpType::StoreEntry))
    .bind(validation_status)
    .bind(author.map(|a| a.get_raw_36().to_vec()))
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The `Delete` actions on `entry_hash` (deletes whose `deletes_entry_hash` is the entry).
pub(crate) async fn get_delete_actions_for_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM DeletedRecord d
         JOIN Action a ON d.action_hash = a.hash
         WHERE d.deletes_entry_hash = ?",
    )
    .bind(entry_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The `Update` actions from `entry_hash` (updates whose `original_entry_hash` is the entry).
pub(crate) async fn get_update_actions_for_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM UpdatedRecord u
         JOIN Action a ON u.action_hash = a.hash
         WHERE u.original_entry_hash = ?",
    )
    .bind(entry_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The `Delete` actions that target `record_action_hash` (its CRUD deletes).
pub(crate) async fn get_delete_actions_for_record<'e, E>(
    executor: E,
    record_action_hash: &ActionHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM DeletedRecord d
         JOIN Action a ON d.action_hash = a.hash
         WHERE d.deletes_action_hash = ?",
    )
    .bind(record_action_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The `Update` actions that update `record_action_hash` (its CRUD updates).
pub(crate) async fn get_update_actions_for_record<'e, E>(
    executor: E,
    record_action_hash: &ActionHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM UpdatedRecord u
         JOIN Action a ON u.action_hash = a.hash
         WHERE u.original_action_hash = ?",
    )
    .bind(record_action_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The live `CreateLink` actions on `base` — link-index rows for the base whose
/// create has no `DeletedLink` tombstone. Returns the full `CreateLink` actions
/// so callers can build `Link`s and filter by type/tag/author/time.
pub(crate) async fn get_live_link_actions<'e, E>(
    executor: E,
    base: &AnyLinkableHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM Link l
         JOIN Action a ON l.action_hash = a.hash
         WHERE l.base_hash = ?
           AND NOT EXISTS (SELECT 1 FROM DeletedLink d WHERE d.create_link_hash = l.action_hash)",
    )
    .bind(base.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// Return actions whose `prev_hash = :prev_hash` and `hash != :exclude_hash`.
/// Used by the sys-validation workflow to detect chain forks.
pub(crate) async fn get_actions_by_prev_hash<'e, E>(
    executor: E,
    prev_hash: &ActionHash,
    exclude_hash: &ActionHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT hash, author, seq, prev_hash, timestamp, action_type,
                action_data, signature, entry_hash, private_entry, record_validity
         FROM Action WHERE prev_hash = ? AND hash != ?",
    )
    .bind(prev_hash.get_raw_36())
    .bind(exclude_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}
