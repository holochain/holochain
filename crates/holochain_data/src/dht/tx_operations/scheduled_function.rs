//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `ScheduledFunction` table.

use super::super::inner::scheduled_function::{self, InsertScheduledFunction};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use holo_hash::AgentPubKey;
use holochain_timestamp::Timestamp;

impl TxRead<Dht> {
    /// Fetch persisted (non-ephemeral) scheduled-function rows for `author` whose
    /// `end_at` is before `now`. Returns `(zome_name, scheduled_fn, maybe_schedule_blob)` tuples.
    pub async fn get_expired_persisted_scheduled_functions(
        &mut self,
        author: &AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<Vec<(String, String, Vec<u8>)>> {
        scheduled_function::get_expired_persisted_scheduled_functions(self.conn_mut(), author, now)
            .await
    }
}

impl TxWrite<Dht> {
    /// Upsert a scheduled-function row. Returns the number of rows written.
    pub async fn upsert_scheduled_function(
        &mut self,
        f: InsertScheduledFunction<'_>,
    ) -> sqlx::Result<u64> {
        scheduled_function::upsert_scheduled_function(self.conn_mut(), f).await
    }

    /// Delete the scheduled-function row for the given `(author, zome_name, scheduled_fn)` tuple.
    /// Returns the number of rows deleted.
    pub async fn delete_scheduled_function(
        &mut self,
        author: &AgentPubKey,
        zome_name: &str,
        scheduled_fn: &str,
    ) -> sqlx::Result<u64> {
        scheduled_function::delete_scheduled_function(
            self.conn_mut(),
            author,
            zome_name,
            scheduled_fn,
        )
        .await
    }

    /// Delete all live ephemeral scheduled-function rows for `author` whose
    /// `start_at` is at or before `now`. Returns the number of rows deleted.
    pub async fn delete_live_ephemeral_scheduled_functions(
        &mut self,
        author: &AgentPubKey,
        now: Timestamp,
    ) -> sqlx::Result<u64> {
        scheduled_function::delete_live_ephemeral_scheduled_functions(self.conn_mut(), author, now)
            .await
    }
}
