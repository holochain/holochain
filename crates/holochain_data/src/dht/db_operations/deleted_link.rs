//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `DeletedLink` table.

use super::super::inner::deleted_link::{self, InsertDeletedLink};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::DeletedLinkRow;
use holo_hash::ActionHash;

impl DbWrite<Dht> {
    /// Insert a row into the `DeletedLink` index table. Returns the number of rows inserted.
    pub async fn insert_deleted_link_index(
        &self,
        link: InsertDeletedLink<'_>,
    ) -> sqlx::Result<u64> {
        deleted_link::insert_deleted_link_index(self.pool(), link).await
    }
}

impl DbRead<Dht> {
    pub async fn get_deleted_links(
        &self,
        create_link_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedLinkRow>> {
        deleted_link::get_deleted_links(self.pool(), create_link_hash).await
    }
}
