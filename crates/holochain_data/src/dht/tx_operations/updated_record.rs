//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `UpdatedRecord` table.

use super::super::inner::updated_record;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::UpdatedRecordRow;
use holo_hash::{ActionHash, EntryHash};

impl TxWrite<Dht> {
    pub async fn insert_updated_record_index(
        &mut self,
        action_hash: &ActionHash,
        original_action_hash: &ActionHash,
        original_entry_hash: &EntryHash,
    ) -> sqlx::Result<()> {
        updated_record::insert_updated_record_index(
            self.conn_mut(),
            action_hash,
            original_action_hash,
            original_entry_hash,
        )
        .await
    }
}

impl TxRead<Dht> {
    pub async fn get_updated_records(
        &mut self,
        original_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<UpdatedRecordRow>> {
        updated_record::get_updated_records(self.conn_mut(), original_action_hash).await
    }
}
