//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `DeletedRecord` table.

use super::super::inner::deleted_record;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::DeletedRecordRow;
use holo_hash::{ActionHash, EntryHash};

impl TxWrite<Dht> {
    pub async fn insert_deleted_record_index(
        &mut self,
        action_hash: &ActionHash,
        deletes_action_hash: &ActionHash,
        deletes_entry_hash: &EntryHash,
    ) -> sqlx::Result<()> {
        deleted_record::insert_deleted_record_index(
            self.conn_mut(),
            action_hash,
            deletes_action_hash,
            deletes_entry_hash,
        )
        .await
    }
}

impl TxRead<Dht> {
    pub async fn get_deleted_records(
        &mut self,
        deletes_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedRecordRow>> {
        deleted_record::get_deleted_records(self.conn_mut(), deletes_action_hash).await
    }
}
