//! `DbRead<Dht>` API for the `DeletedRecord` table.
//!
//! `insert_deleted_record_index` is intentionally only on `TxWrite<Dht>` —
//! see [`super::super::tx_operations::deleted_record`].

use super::super::inner::deleted_record;
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::DeletedRecordRow;
use holo_hash::ActionHash;

impl DbRead<Dht> {
    pub async fn get_deleted_records(
        &self,
        deletes_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedRecordRow>> {
        deleted_record::get_deleted_records(self.pool(), deletes_action_hash).await
    }
}
