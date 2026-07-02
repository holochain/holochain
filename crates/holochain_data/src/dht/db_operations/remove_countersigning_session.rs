//! `DbWrite<Dht>` API for force-removing a self-authored countersigning
//! session.

use super::super::inner::remove_countersigning_session::{
    self, RemoveCountersigningSessionOutcome,
};
use crate::handles::DbWrite;
use crate::kind::Dht;
use holo_hash::{ActionHash, EntryHash};

impl DbWrite<Dht> {
    /// Force-remove a self-authored countersigning session, wrapped in a single
    /// transaction so the published guard and the deletes are atomic. See the
    /// inner `remove_countersigning_session` for the guard and deletion
    /// semantics.
    pub async fn remove_countersigning_session(
        &self,
        action_hash: &ActionHash,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<RemoveCountersigningSessionOutcome> {
        let mut tx = self.begin().await?;
        let outcome = remove_countersigning_session::remove_countersigning_session(
            tx.conn_mut(),
            action_hash,
            entry_hash,
        )
        .await?;
        tx.commit().await?;
        Ok(outcome)
    }
}
