//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `ValidationReceipt` table.

use super::super::inner::validation_receipt;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::{ValidationReceiptForActionRow, ValidationReceiptRow};
use holo_hash::{ActionHash, DhtOpHash};
use holochain_timestamp::Timestamp;

impl DbWrite<Dht> {
    pub async fn insert_validation_receipt(
        &self,
        hash: &DhtOpHash,
        op_hash: &DhtOpHash,
        blob: &[u8],
        when_received: Timestamp,
    ) -> sqlx::Result<()> {
        validation_receipt::insert_validation_receipt(
            self.pool(),
            hash,
            op_hash,
            blob,
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

    /// Validation receipts for every op of `action_hash`, joined with op type
    /// and publish-completion flag. See
    /// [`validation_receipt::validation_receipts_for_action`].
    pub async fn validation_receipts_for_action(
        &self,
        action_hash: ActionHash,
    ) -> sqlx::Result<Vec<ValidationReceiptForActionRow>> {
        validation_receipt::validation_receipts_for_action(self.pool(), action_hash).await
    }
}
