//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `ValidationReceipt` table.

use super::super::inner::validation_receipt;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::ValidationReceiptRow;
use holo_hash::DhtOpHash;
use holochain_timestamp::Timestamp;

impl TxWrite<Dht> {
    pub async fn insert_validation_receipt(
        &mut self,
        hash: &DhtOpHash,
        op_hash: &DhtOpHash,
        validators: &[u8],
        signature: &[u8],
        when_received: Timestamp,
    ) -> sqlx::Result<()> {
        validation_receipt::insert_validation_receipt(
            self.conn_mut(),
            hash,
            op_hash,
            validators,
            signature,
            when_received,
        )
        .await
    }
}

impl TxRead<Dht> {
    pub async fn get_validation_receipts(
        &mut self,
        op_hash: DhtOpHash,
    ) -> sqlx::Result<Vec<ValidationReceiptRow>> {
        validation_receipt::get_validation_receipts(self.conn_mut(), op_hash).await
    }
}
