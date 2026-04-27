//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `WarrantPublish` table.

use super::super::inner::warrant_publish;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::WarrantPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_warrant_publish(
        &self,
        warrant_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
    ) -> sqlx::Result<()> {
        warrant_publish::insert_warrant_publish(self.pool(), warrant_hash, last_publish_time).await
    }
}

impl DbRead<Dht> {
    pub async fn get_warrant_publish(
        &self,
        warrant_hash: DhtOpHash,
    ) -> sqlx::Result<Option<WarrantPublishRow>> {
        warrant_publish::get_warrant_publish(self.pool(), warrant_hash).await
    }
}
