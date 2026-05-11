//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `ChainOpPublish` table.

use super::super::inner::chain_op_publish;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::ChainOpPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl TxWrite<Dht> {
    pub async fn insert_chain_op_publish(
        &mut self,
        op_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
        receipts_complete: Option<bool>,
        withhold_publish: Option<bool>,
    ) -> sqlx::Result<()> {
        chain_op_publish::insert_chain_op_publish(
            self.conn_mut(),
            op_hash,
            last_publish_time,
            receipts_complete,
            withhold_publish,
        )
        .await
    }

    /// Mark receipts as complete for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_receipts_complete(
        &mut self,
        op_hash: &DhtOpHash,
    ) -> sqlx::Result<u64> {
        chain_op_publish::set_receipts_complete(self.conn_mut(), op_hash).await
    }

    /// Update `last_publish_time` for the given op. Returns the number of rows updated.
    pub async fn set_chain_op_last_publish_time(
        &mut self,
        op_hash: &DhtOpHash,
        when: Timestamp,
    ) -> sqlx::Result<u64> {
        chain_op_publish::set_last_publish_time(self.conn_mut(), op_hash, when).await
    }

    /// Clear `withhold_publish` (set to NULL) for the given op. Returns the number of rows updated.
    pub async fn clear_chain_op_withhold_publish(
        &mut self,
        op_hash: &DhtOpHash,
    ) -> sqlx::Result<u64> {
        chain_op_publish::clear_withhold_publish(self.conn_mut(), op_hash).await
    }
}

impl TxRead<Dht> {
    pub async fn get_chain_op_publish(
        &mut self,
        op_hash: DhtOpHash,
    ) -> sqlx::Result<Option<ChainOpPublishRow>> {
        chain_op_publish::get_chain_op_publish(self.conn_mut(), op_hash).await
    }
}
