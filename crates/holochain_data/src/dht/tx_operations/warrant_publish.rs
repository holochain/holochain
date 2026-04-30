//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `WarrantPublish` table.

use super::super::inner::warrant_publish;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::WarrantPublishRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl TxWrite<Dht> {
    pub async fn insert_warrant_publish(
        &mut self,
        warrant_hash: &DhtOpHash,
        last_publish_time: Option<Timestamp>,
    ) -> sqlx::Result<()> {
        warrant_publish::insert_warrant_publish(self.conn_mut(), warrant_hash, last_publish_time)
            .await
    }
}

impl TxRead<Dht> {
    pub async fn get_warrant_publish(
        &mut self,
        warrant_hash: DhtOpHash,
    ) -> sqlx::Result<Option<WarrantPublishRow>> {
        warrant_publish::get_warrant_publish(self.conn_mut(), warrant_hash).await
    }
}
