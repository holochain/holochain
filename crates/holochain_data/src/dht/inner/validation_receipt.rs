//! Free-standing operations against the `ValidationReceipt` table.

use crate::models::dht::{ValidationReceiptForActionRow, ValidationReceiptRow};
use holo_hash::{ActionHash, DhtOpHash};
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_validation_receipt<'e, E>(
    executor: E,
    hash: &DhtOpHash,
    op_hash: &DhtOpHash,
    blob: &[u8],
    when_received: Timestamp,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ValidationReceipt (hash, op_hash, blob, when_received)
         VALUES (?, ?, ?, ?)",
    )
    .bind(hash.get_raw_36())
    .bind(op_hash.get_raw_36())
    .bind(blob)
    .bind(when_received.as_micros())
    .execute(executor)
    .await?;
    Ok(())
}

pub(crate) async fn get_validation_receipts<'e, E>(
    executor: E,
    op_hash: DhtOpHash,
) -> sqlx::Result<Vec<ValidationReceiptRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT hash, op_hash, blob, when_received
         FROM ValidationReceipt WHERE op_hash = ? ORDER BY when_received",
    )
    .bind(op_hash.get_raw_36())
    .fetch_all(executor)
    .await
}

/// Validation receipts for every op of a given action, joined with the op type
/// and publish completion flag. Used to build the `get_validation_receipts`
/// host-function response.
pub(crate) async fn validation_receipts_for_action<'e, E>(
    executor: E,
    action_hash: ActionHash,
) -> sqlx::Result<Vec<ValidationReceiptForActionRow>>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query_as(
        "SELECT vr.blob              AS receipt_blob,
                co.hash              AS op_hash,
                co.op_type           AS op_type,
                cop.receipts_complete AS receipts_complete
         FROM Action a
         JOIN ChainOp co ON co.action_hash = a.hash
         JOIN ValidationReceipt vr ON vr.op_hash = co.hash
         LEFT JOIN ChainOpPublish cop ON cop.op_hash = co.hash
         WHERE a.hash = ?",
    )
    .bind(action_hash.get_raw_36())
    .fetch_all(executor)
    .await
}
