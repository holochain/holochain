//! `TxWrite<Dht>` API for force-removing a self-authored countersigning
//! session within an in-flight transaction.

use super::super::inner::remove_countersigning_session::{
    self, RemoveCountersigningSessionOutcome,
};
use crate::handles::TxWrite;
use crate::kind::Dht;
use holo_hash::{ActionHash, EntryHash};

impl TxWrite<Dht> {
    /// Force-remove a self-authored countersigning session using the caller's
    /// transaction. See
    /// [`remove_countersigning_session::remove_countersigning_session`] for the
    /// guard and deletion semantics; commit or rollback is the caller's
    /// responsibility.
    pub async fn remove_countersigning_session(
        &mut self,
        action_hash: &ActionHash,
        entry_hash: &EntryHash,
    ) -> sqlx::Result<RemoveCountersigningSessionOutcome> {
        remove_countersigning_session::remove_countersigning_session(
            self.conn_mut(),
            action_hash,
            entry_hash,
        )
        .await
    }
}
