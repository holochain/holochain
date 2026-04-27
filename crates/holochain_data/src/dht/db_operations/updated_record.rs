//! `DbRead<Dht>` API for the `UpdatedRecord` table.

use super::super::inner::updated_record;
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::UpdatedRecordRow;
use holo_hash::ActionHash;

impl DbRead<Dht> {
    pub async fn get_updated_records(
        &self,
        original_action_hash: ActionHash,
    ) -> sqlx::Result<Vec<UpdatedRecordRow>> {
        updated_record::get_updated_records(self.pool(), original_action_hash).await
    }
}
