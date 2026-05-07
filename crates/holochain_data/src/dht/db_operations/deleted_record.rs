//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `DeletedRecord` table.

use super::super::inner::deleted_record::{self, InsertDeletedRecord};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::DeletedRecordRow;
use holo_hash::ActionHash;

impl DbWrite<Dht> {
    /// Insert a row into the `DeletedRecord` index table. Returns the number of rows inserted.
    pub async fn insert_deleted_record_index(
        &self,
        record: InsertDeletedRecord<'_>,
    ) -> sqlx::Result<u64> {
        deleted_record::insert_deleted_record_index(self.pool(), record).await
    }
}

impl DbRead<Dht> {
    pub async fn get_deleted_records(
        &self,
        deletes_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedRecordRow>> {
        deleted_record::get_deleted_records(self.pool(), deletes_action_hash).await
    }
}
