//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `DeletedLink` table.

use super::super::inner::deleted_link::{self, InsertDeletedLink};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::DeletedLinkRow;
use holo_hash::ActionHash;

impl TxWrite<Dht> {
    /// Insert a row into the `DeletedLink` index table. Returns the number of rows inserted.
    pub async fn insert_deleted_link_index(
        &mut self,
        link: InsertDeletedLink<'_>,
    ) -> sqlx::Result<u64> {
        deleted_link::insert_deleted_link_index(self.conn_mut(), link).await
    }
}

impl TxRead<Dht> {
    pub async fn get_deleted_links(
        &mut self,
        create_link_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedLinkRow>> {
        deleted_link::get_deleted_links(self.conn_mut(), create_link_hash).await
    }
}
