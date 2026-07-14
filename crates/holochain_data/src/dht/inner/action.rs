//! Free-standing operations against the `Action` table.

use crate::models::dht::{ActionRow, AgentActivityItem, AgentActivityRow, ValidatedActionRow};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, EntryHash, HoloHashed};
use holochain_integrity_types::action::{Action, ActionData, ActionHeader, RecordValidity};
use holochain_integrity_types::entry::Entry;
use holochain_integrity_types::entry_def::EntryVisibility;
use holochain_integrity_types::record::SignedHashed;
use holochain_integrity_types::signature::Signature;
use holochain_zome_types::action::SignedActionHashed;
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

/// The author's committed source chain, ordered by sequence.
///
/// Restricted to accepted rows (`record_validity = Accepted`): integrated
/// actions whose ops all pass validation. Pending (limbo) and rejected rows are
/// excluded, so this agrees with [`chain_head_for_author`].
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
         FROM Action WHERE author = ? AND record_validity = ? ORDER BY seq ASC",
    )
    .bind(author.get_raw_36())
    .bind(i64::from(RecordValidity::Accepted))
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// Count actions authored by `author`, stopping once `cap` rows have been
/// counted. Used by the genesis check, which only needs to know whether the
/// first few genesis actions are present, not the full chain length.
pub(crate) async fn count_author_actions_capped<'e, E>(
    executor: E,
    author: &AgentPubKey,
    cap: i64,
) -> sqlx::Result<i64>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT 1 FROM Action WHERE author = ? LIMIT ?)")
        .bind(author.get_raw_36())
        .bind(cap)
        .fetch_one(executor)
        .await
}

/// The author's committed source-chain head: the highest-sequence action they
/// authored that is marked accepted. Returns `None` for an empty chain
/// (pre-genesis).
///
/// Acceptability is read from the `Action` row's own `record_validity` state,
/// not by joining to an op row, so the result never depends on holding a
/// particular op such as `RegisterAgentActivity`. `record_validity` is the
/// action's aggregated integration status: a self-authored action is `Accepted`
/// when the flush writes it, and a network-received action becomes `Accepted`
/// once its ops integrate. Pending (limbo) and rejected actions are excluded, so
/// a forged high-sequence action — which cannot pass validation to reach
/// `Accepted` — cannot falsely trip the flush as-at / head-moved check. The
/// flush writes the action and its ops in one transaction, so a freshly
/// committed head is immediately visible here. Withheld in-flight countersigning
/// actions are self-authored and keep `Accepted`, so they remain part of the
/// head; only their publishing is withheld.
pub(crate) async fn chain_head_for_author<'e, E>(
    executor: E,
    author: &AgentPubKey,
) -> sqlx::Result<Option<(ActionHash, u32, holochain_timestamp::Timestamp)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(Vec<u8>, i64, i64)> = sqlx::query_as(
        "SELECT hash, seq, timestamp FROM Action
         WHERE author = ? AND record_validity = ?
         ORDER BY seq DESC LIMIT 1",
    )
    .bind(author.get_raw_36())
    .bind(i64::from(RecordValidity::Accepted))
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(hash_bytes, seq, ts)| {
        (
            ActionHash::from_raw_36(hash_bytes),
            seq as u32,
            holochain_timestamp::Timestamp::from_micros(ts),
        )
    }))
}

/// All actions authored by `author` that have an integrated
/// `RegisterAgentActivity` op, ordered by chain sequence. When
/// `include_entries` is set, the public
/// `Entry` blob is joined in (Full mode); otherwise the entry column is `NULL`.
///
/// Ops withheld from publishing (in-flight countersigning sessions, where
/// `ChainOpPublish.withhold_publish` is set) are excluded: such an op is
/// authored locally but must not surface as live agent activity until the
/// session completes and the withhold flag is cleared. The `LEFT JOIN` leaves
/// ordinary ops — which have either no `ChainOpPublish` row (network/other-agent
/// activity) or a row with `withhold_publish` `NULL` — unaffected.
pub(crate) async fn get_agent_activity<'e, E>(
    executor: E,
    author: &AgentPubKey,
    include_entries: bool,
) -> sqlx::Result<Vec<AgentActivityItem>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = if include_entries {
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status, e.blob AS entry_blob
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         LEFT JOIN Entry e ON a.entry_hash = e.hash
         LEFT JOIN ChainOpPublish cp ON cp.op_hash = c.hash
         WHERE a.author = ? AND c.op_type = ? AND cp.withhold_publish IS NULL
         ORDER BY a.seq ASC"
    } else {
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status, NULL AS entry_blob
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         LEFT JOIN ChainOpPublish cp ON cp.op_hash = c.hash
         WHERE a.author = ? AND c.op_type = ? AND cp.withhold_publish IS NULL
         ORDER BY a.seq ASC"
    };
    let rows: Vec<AgentActivityRow> = sqlx::query_as(sql)
        .bind(author.get_raw_36())
        .bind(i64::from(ChainOpType::RegisterAgentActivity))
        .fetch_all(executor)
        .await?;
    rows.into_iter().map(agent_activity_row_to_item).collect()
}

/// Bounded `RegisterAgentActivity` scan for `must_get_agent_activity`: integrated
/// actions authored by `author` with `seq <= chain_top_seq` and (when
/// `until_seq` is `Some`) `seq >= until_seq`, ordered by `seq DESC, hash DESC`.
pub(crate) async fn get_filtered_agent_activity<'e, E>(
    executor: E,
    author: &AgentPubKey,
    chain_top_seq: u32,
    until_seq: Option<u32>,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         WHERE c.op_type = ?
           AND a.author = ?
           AND a.seq <= ?
           AND (? IS NULL OR a.seq >= ?)
         ORDER BY a.seq DESC, a.hash DESC",
    )
    .bind(i64::from(ChainOpType::RegisterAgentActivity))
    .bind(author.get_raw_36())
    .bind(chain_top_seq as i64)
    .bind(until_seq.map(|s| s as i64))
    .bind(until_seq.map(|s| s as i64))
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The chain sequence and authored timestamp of `action_hash`, if it is an
/// integrated `RegisterAgentActivity` action authored by `author`.
pub(crate) async fn get_action_seq_and_timestamp<'e, E>(
    executor: E,
    author: &AgentPubKey,
    action_hash: &ActionHash,
) -> sqlx::Result<Option<(u32, holochain_timestamp::Timestamp)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT a.seq, a.timestamp
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         WHERE a.hash = ? AND a.author = ? AND c.op_type = ?",
    )
    .bind(action_hash.get_raw_36())
    .bind(author.get_raw_36())
    .bind(i64::from(ChainOpType::RegisterAgentActivity))
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(seq, ts)| (seq as u32, holochain_timestamp::Timestamp::from_micros(ts))))
}

fn agent_activity_row_to_item(row: AgentActivityRow) -> sqlx::Result<AgentActivityItem> {
    let action = row_to_signed_action_hashed(row.action)?;
    let validation_status = RecordValidity::try_from(row.validation_status).map_err(|v| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid validation_status {v} on RegisterAgentActivity op"),
        )))
    })?;
    let entry = match row.entry_blob {
        Some(blob) => Some(
            holochain_serialized_bytes::decode::<_, Entry>(&blob).map_err(|e| {
                sqlx::Error::Decode(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("decode Entry: {e}"),
                )))
            })?,
        ),
        None => None,
    };
    Ok(AgentActivityItem {
        action,
        validation_status,
        entry,
    })
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
           AND (a.private_entry = 0 OR a.private_entry IS NULL OR a.author = ?)",
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
pub(crate) async fn get_create_actions_for_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
    author: Option<&AgentPubKey>,
    validation_status: RecordValidity,
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
           AND (a.private_entry = 0 OR a.private_entry IS NULL OR a.author = ?)",
    )
    .bind(entry_hash.get_raw_36())
    .bind(i64::from(ChainOpType::StoreEntry))
    .bind(i64::from(validation_status))
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

/// All `CreateLink` actions on `base` (live AND tombstoned), for link details.
pub(crate) async fn get_link_create_actions<'e, E>(
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
         WHERE l.base_hash = ?",
    )
    .bind(base.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// The `DeleteLink` actions that tombstone `create_link_hash`.
pub(crate) async fn get_delete_link_actions<'e, E>(
    executor: E,
    create_link_hash: &ActionHash,
) -> sqlx::Result<Vec<SignedActionHashed>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity
         FROM DeletedLink d
         JOIN Action a ON d.action_hash = a.hash
         WHERE d.create_link_hash = ?",
    )
    .bind(create_link_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(row_to_signed_action_hashed).collect()
}

/// Decode a `ValidatedActionRow` to an action + its validation status.
fn validated_action_row_to_item(
    row: ValidatedActionRow,
) -> sqlx::Result<(SignedActionHashed, RecordValidity)> {
    let action = row_to_signed_action_hashed(row.action)?;
    let validation_status = RecordValidity::try_from(row.validation_status).map_err(|v| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid validation_status {v} on ChainOp"),
        )))
    })?;
    Ok((action, validation_status))
}

/// Authority-serving create-link actions for `base`: locally-validated
/// (`locally_validated = 1`) `RegisterAddLink` ops only, each with its
/// validation status. Cached links (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_link_creates<'e, E>(
    executor: E,
    base: &AnyLinkableHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM Link l
         JOIN Action a ON l.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE l.base_hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::RegisterAddLink))
    .bind(base.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving delete-link actions targeting `base`'s links:
/// locally-validated `RegisterRemoveLink` ops only, each with its validation
/// status.
pub(crate) async fn get_authority_delete_links<'e, E>(
    executor: E,
    base: &AnyLinkableHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM DeletedLink d
         JOIN Action a ON d.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE c.locally_validated = 1
           AND d.create_link_hash IN (SELECT l.action_hash FROM Link l WHERE l.base_hash = ?)",
    )
    .bind(i64::from(ChainOpType::RegisterRemoveLink))
    .bind(base.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving `StoreRecord` action for `action_hash`: present only if a
/// locally-validated (`locally_validated = 1`) `StoreRecord` op exists for it,
/// with its validation status.
pub(crate) async fn get_authority_store_record<'e, E>(
    executor: E,
    action_hash: &ActionHash,
) -> sqlx::Result<Option<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let row: Option<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM Action a
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE a.hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::StoreRecord))
    .bind(action_hash.get_raw_36())
    .fetch_optional(executor)
    .await?;
    row.map(validated_action_row_to_item).transpose()
}

/// Authority-serving delete actions targeting record `record_action_hash`:
/// locally-validated `RegisterDeletedBy` ops only, each with its validation status.
/// Cached ops (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_deletes_for_record<'e, E>(
    executor: E,
    record_action_hash: &ActionHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM DeletedRecord d
         JOIN Action a ON d.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE d.deletes_action_hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::RegisterDeletedBy))
    .bind(record_action_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving update actions targeting record `record_action_hash`:
/// locally-validated `RegisterUpdatedRecord` ops only, each with its validation status.
/// Cached ops (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_updates_for_record<'e, E>(
    executor: E,
    record_action_hash: &ActionHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM UpdatedRecord u
         JOIN Action a ON u.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE u.original_action_hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::RegisterUpdatedRecord))
    .bind(record_action_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving create actions for entry `entry_hash`: locally-validated
/// `StoreEntry` ops only, each with its validation status.
/// Cached ops (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_entry_creates<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM ChainOp c
         JOIN Action a ON c.action_hash = a.hash
         WHERE c.basis_hash = ? AND c.op_type = ? AND c.locally_validated = 1",
    )
    .bind(entry_hash.get_raw_36())
    .bind(i64::from(ChainOpType::StoreEntry))
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving delete actions targeting entry `entry_hash`:
/// locally-validated `RegisterDeletedEntryAction` ops only, each with its validation status.
/// Cached ops (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_deletes_for_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM DeletedRecord d
         JOIN Action a ON d.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE d.deletes_entry_hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::RegisterDeletedEntryAction))
    .bind(entry_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
}

/// Authority-serving update actions targeting entry `entry_hash`:
/// locally-validated `RegisterUpdatedContent` ops only, each with its validation status.
/// Cached ops (`locally_validated = 0`) are excluded.
pub(crate) async fn get_authority_updates_for_entry<'e, E>(
    executor: E,
    entry_hash: &EntryHash,
) -> sqlx::Result<Vec<(SignedActionHashed, RecordValidity)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    let rows: Vec<ValidatedActionRow> = sqlx::query_as(
        "SELECT a.hash, a.author, a.seq, a.prev_hash, a.timestamp, a.action_type,
                a.action_data, a.signature, a.entry_hash, a.private_entry, a.record_validity,
                c.validation_status
         FROM UpdatedRecord u
         JOIN Action a ON u.action_hash = a.hash
         JOIN ChainOp c ON c.action_hash = a.hash AND c.op_type = ?
         WHERE u.original_entry_hash = ? AND c.locally_validated = 1",
    )
    .bind(i64::from(ChainOpType::RegisterUpdatedContent))
    .bind(entry_hash.get_raw_36())
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(validated_action_row_to_item).collect()
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
