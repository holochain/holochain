//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `UpdatedRecord` table.

use super::super::inner::updated_record::{self, InsertUpdatedRecord};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::UpdatedRecordRow;
use holo_hash::ActionHash;

impl DbWrite<Dht> {
    /// Insert a row into the `UpdatedRecord` index table. Returns the number of rows inserted.
    pub async fn insert_updated_record_index(
        &self,
        record: InsertUpdatedRecord<'_>,
    ) -> sqlx::Result<u64> {
        updated_record::insert_updated_record_index(self.pool(), record).await
    }
}

impl DbRead<Dht> {
    pub async fn get_updated_records(
        &self,
        original_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<UpdatedRecordRow>> {
        updated_record::get_updated_records(self.pool(), original_action_hash).await
    }
}
