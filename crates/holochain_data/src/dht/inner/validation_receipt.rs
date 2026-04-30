//! Free-standing operations against the `ValidationReceipt` table.

use crate::models::dht::ValidationReceiptRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

pub(crate) async fn insert_validation_receipt<'e, E>(
    executor: E,
    hash: &DhtOpHash,
    op_hash: &DhtOpHash,
    validators: &[u8],
    signature: &[u8],
    when_received: Timestamp,
) -> sqlx::Result<()>
where
    E: Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO ValidationReceipt (hash, op_hash, validators, signature, when_received)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(hash.get_raw_36())
    .bind(op_hash.get_raw_36())
    .bind(validators)
    .bind(signature)
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
        "SELECT hash, op_hash, validators, signature, when_received
         FROM ValidationReceipt WHERE op_hash = ? ORDER BY when_received",
    )
    .bind(op_hash.get_raw_36())
    .fetch_all(executor)
    .await
}
