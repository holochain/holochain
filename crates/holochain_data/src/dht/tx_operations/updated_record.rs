//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `UpdatedRecord` table.

use super::super::inner::updated_record::{self, InsertUpdatedRecord};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::UpdatedRecordRow;
use holo_hash::ActionHash;

impl TxWrite<Dht> {
    /// Insert a row into the `UpdatedRecord` index table. Returns the number of rows inserted.
    pub async fn insert_updated_record_index(
        &mut self,
        record: InsertUpdatedRecord<'_>,
    ) -> sqlx::Result<u64> {
        updated_record::insert_updated_record_index(self.conn_mut(), record).await
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
