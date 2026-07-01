//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ChainOpPublish` table.

use super::super::inner::chain_op_publish;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::{ChainOpPublishRow, OpToPublishRow};
use holo_hash::{AgentPubKey, DhtOpHash};
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
        let mut conn = self.timed_conn().await?;
        chain_op_publish::get_chain_op_publish(&mut *conn, op_hash).await
    }

    /// Count integrated ops authored by `author` that have been published at
    /// least once. Used by the source-chain dump to compute `published_ops_count`.
    pub async fn count_published_ops_for_author(&self, author: &AgentPubKey) -> sqlx::Result<i64> {
        let mut conn = self.timed_conn().await?;
        chain_op_publish::count_published_ops_for_author(&mut *conn, author).await
    }

    /// Ops eligible to be published for `author`. See
    /// `chain_op_publish::get_ops_to_publish`.
    pub async fn get_ops_to_publish(
        &self,
        author: &AgentPubKey,
        recency_threshold_micros: i64,
    ) -> sqlx::Result<Vec<OpToPublishRow>> {
        let mut conn = self.timed_conn().await?;
        chain_op_publish::get_ops_to_publish(&mut *conn, author, recency_threshold_micros).await
    }

    /// Count ops authored by `author` that may still need publishing. See
    /// `chain_op_publish::num_still_needing_publish`.
    pub async fn num_still_needing_publish(&self, author: &AgentPubKey) -> sqlx::Result<i64> {
        let mut conn = self.timed_conn().await?;
        chain_op_publish::num_still_needing_publish(&mut *conn, author).await
    }
}
