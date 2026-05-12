//! Free-standing operations against the `ScheduledFunction` table.

use holo_hash::AgentPubKey;
use holochain_timestamp::Timestamp;
use sqlx::{Executor, Sqlite};

/// Parameters for inserting (or upserting) a row into `ScheduledFunction`.
pub struct InsertScheduledFunction<'a> {
    /// Agent that owns this scheduled function.
    pub author: &'a AgentPubKey,
    /// Name of the zome containing the scheduled function.
    pub zome_name: &'a str,
    /// Name of the scheduled function within the zome.
    pub scheduled_fn: &'a str,
    /// Serialized `Option<Schedule>` blob — the same encoding the legacy
    /// table uses for `maybe_schedule`.
    pub maybe_schedule: &'a [u8],
    /// Microsecond timestamp at which the function becomes live.
    pub start_at: Timestamp,
    /// Microsecond timestamp at which the function expires.
    pub end_at: Timestamp,
    /// `true` if the row is removed once the function fires.
    pub ephemeral: bool,
}

/// Upsert a scheduled-function row, updating existing fields when the
/// `(author, zome_name, scheduled_fn)` primary key already exists.
/// Returns the number of rows written (1 on insert or update, 0 on no-op).
pub(crate) async fn upsert_scheduled_function<'a, 'e, E>(
    executor: E,
    f: InsertScheduledFunction<'a>,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "INSERT INTO ScheduledFunction
            (author, zome_name, scheduled_fn, maybe_schedule, start_at, end_at, ephemeral)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(author, zome_name, scheduled_fn) DO UPDATE SET
             maybe_schedule = excluded.maybe_schedule,
             start_at       = excluded.start_at,
             end_at         = excluded.end_at,
             ephemeral      = excluded.ephemeral",
    )
    .bind(f.author.get_raw_36())
    .bind(f.zome_name)
    .bind(f.scheduled_fn)
    .bind(f.maybe_schedule)
    .bind(f.start_at.as_micros())
    .bind(f.end_at.as_micros())
    .bind(f.ephemeral as i64)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Delete the scheduled-function row for the given `(author, zome_name, scheduled_fn)` tuple.
///
/// Returns the number of rows deleted (0 if the row did not exist).
pub(crate) async fn delete_scheduled_function<'e, E>(
    executor: E,
    author: &AgentPubKey,
    zome_name: &str,
    scheduled_fn: &str,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "DELETE FROM ScheduledFunction
         WHERE author = ? AND zome_name = ? AND scheduled_fn = ?",
    )
    .bind(author.get_raw_36())
    .bind(zome_name)
    .bind(scheduled_fn)
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

/// Return persisted (non-ephemeral) scheduled-function rows for `author` whose
/// `end_at` is before `now`, as `(zome_name, scheduled_fn, maybe_schedule_blob)` tuples.
pub(crate) async fn get_expired_persisted_scheduled_functions<'e, E>(
    executor: E,
    author: &AgentPubKey,
    now: Timestamp,
) -> sqlx::Result<Vec<(String, String, Vec<u8>)>>
where
    E: Executor<'e, Database = Sqlite>,
{
    #[derive(sqlx::FromRow)]
    struct Row {
        zome_name: String,
        scheduled_fn: String,
        maybe_schedule: Vec<u8>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT zome_name, scheduled_fn, maybe_schedule
         FROM ScheduledFunction
         WHERE ephemeral = 0 AND author = ? AND end_at < ?",
    )
    .bind(author.get_raw_36())
    .bind(now.as_micros())
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.zome_name, r.scheduled_fn, r.maybe_schedule))
        .collect())
}

/// Delete all live ephemeral scheduled-function rows for `author` whose
/// `start_at` is at or before `now`. Returns the number of rows deleted.
pub(crate) async fn delete_live_ephemeral_scheduled_functions<'e, E>(
    executor: E,
    author: &AgentPubKey,
    now: Timestamp,
) -> sqlx::Result<u64>
where
    E: Executor<'e, Database = Sqlite>,
{
    let result = sqlx::query(
        "DELETE FROM ScheduledFunction
         WHERE ephemeral = 1 AND author = ? AND start_at <= ?",
    )
    .bind(author.get_raw_36())
    .bind(now.as_micros())
    .execute(executor)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Dht;
    use crate::test_open_db;
    use holo_hash::DnaHash;
    use std::sync::Arc;

    fn dht_id() -> Dht {
        Dht::new(Arc::new(DnaHash::from_raw_36(vec![0u8; 36])))
    }

    fn agent(seed: u8) -> AgentPubKey {
        AgentPubKey::from_raw_36(vec![seed; 36])
    }

    #[tokio::test]
    async fn insert_upsert_delete_scheduled_function() {
        let db = test_open_db(dht_id()).await.unwrap();
        let author = agent(1);
        let payload = b"schedule-blob";

        // Initial insert.
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(200),
            ephemeral: true,
        })
        .await
        .unwrap();

        // Same key — the upsert clause should replace the row, not error
        // with a PK conflict.
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(150),
            end_at: Timestamp::from_micros(250),
            ephemeral: false,
        })
        .await
        .unwrap();

        db.delete_scheduled_function(&author, "z", "f")
            .await
            .unwrap();

        // Re-insert succeeds, confirming delete removed the row.
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &author,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(200),
            ephemeral: true,
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn expired_persisted_scoped_to_author() {
        let db = test_open_db(dht_id()).await.unwrap();
        let alice = agent(1);
        let bob = agent(2);
        let payload = b"";

        let now_time = Timestamp::from_micros(200);

        // Alice: persisted, expired (end_at=100, now=200).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(50),
            end_at: Timestamp::from_micros(100),
            ephemeral: false,
        })
        .await
        .unwrap();

        // Alice: persisted, not yet expired (end_at=300, now=200).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "g",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(50),
            end_at: Timestamp::from_micros(300),
            ephemeral: false,
        })
        .await
        .unwrap();

        // Alice: ephemeral, "expired" (must NOT be returned — query is non-ephemeral only).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "e",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(50),
            end_at: Timestamp::from_micros(100),
            ephemeral: true,
        })
        .await
        .unwrap();

        // Bob: persisted, expired but different author (must NOT be returned).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &bob,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(50),
            end_at: Timestamp::from_micros(100),
            ephemeral: false,
        })
        .await
        .unwrap();

        let result = db
            .as_ref()
            .get_expired_persisted_scheduled_functions(&alice, now_time)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "z");
        assert_eq!(result[0].1, "f");
    }

    #[tokio::test]
    async fn delete_live_ephemeral_scoped_to_author_and_now() {
        let db = test_open_db(dht_id()).await.unwrap();
        let alice = agent(1);
        let bob = agent(2);
        let payload = b"";

        // Alice: ephemeral start_at=100 (eligible at now=150).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(300),
            ephemeral: true,
        })
        .await
        .unwrap();
        // Alice: non-ephemeral (must NOT be deleted).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "g",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(300),
            ephemeral: false,
        })
        .await
        .unwrap();
        // Bob: ephemeral but different author (must NOT be deleted).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &bob,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(300),
            ephemeral: true,
        })
        .await
        .unwrap();

        db.delete_live_ephemeral_scheduled_functions(&alice, Timestamp::from_micros(150))
            .await
            .unwrap();

        // Spot-check by re-inserting Alice's ephemeral row to confirm it was
        // gone (otherwise PK conflict would error).
        db.upsert_scheduled_function(InsertScheduledFunction {
            author: &alice,
            zome_name: "z",
            scheduled_fn: "f",
            maybe_schedule: payload,
            start_at: Timestamp::from_micros(100),
            end_at: Timestamp::from_micros(300),
            ephemeral: true,
        })
        .await
        .unwrap();
    }
}
