//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainOpPublish` table.

use super::super::inner::chain_op_publish;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::ChainOpPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_chain_op_publish(
        &self,
        op_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
        receipts_complete: Option<bool>,
        withhold_publish: Option<bool>,
    ) -> sqlx::Result<()> {
        chain_op_publish::insert_chain_op_publish(
            self.pool(),
            op_hash,
            last_publish_time,
            receipts_complete,
            withhold_publish,
        )
        .await
    }

    /// Mark receipts as complete for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_receipts_complete(&self, op_hash: &DhtOpHash) -> sqlx::Result<u64> {
        chain_op_publish::set_receipts_complete(self.pool(), op_hash).await
    }

    /// Update `last_publish_time` for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_last_publish_time(
        &self,
        op_hash: &DhtOpHash,
        when: Timestamp,
    ) -> sqlx::Result<u64> {
        chain_op_publish::set_last_publish_time(self.pool(), op_hash, when).await
    }

    /// Clear `withhold_publish` (set to NULL) for the given op. Returns the number of rows updated.
    pub async fn clear_chain_op_withhold_publish(&self, op_hash: &DhtOpHash) -> sqlx::Result<u64> {
        chain_op_publish::clear_withhold_publish(self.pool(), op_hash).await
    }
}

impl DbRead<Dht> {
    pub async fn get_chain_op_publish(
        &self,
        op_hash: DhtOpHash,
    ) -> sqlx::Result<Option<ChainOpPublishRow>> {
        chain_op_publish::get_chain_op_publish(self.pool(), op_hash).await
    }
}
