//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ValidationReceipt` table.

use super::super::inner::validation_receipt;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::ValidationReceiptRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_validation_receipt(
        &self,
        hash: &DhtOpHash,
        op_hash: &DhtOpHash,
        validators: &[u8],
        signature: &[u8],
        when_received: Timestamp,
    ) -> sqlx::Result<()> {
        validation_receipt::insert_validation_receipt(
            self.pool(),
            hash,
            op_hash,
            validators,
            signature,
            when_received,
        )
        .await
    }
}

impl DbRead<Dht> {
    pub async fn get_validation_receipts(
        &self,
        op_hash: DhtOpHash,
    ) -> sqlx::Result<Vec<ValidationReceiptRow>> {
        validation_receipt::get_validation_receipts(self.pool(), op_hash).await
    }
}
