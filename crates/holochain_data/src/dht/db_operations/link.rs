//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Link` table.

use super::super::inner::link::{self, InsertLink};
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use crate::models::dht::LinkRow;
use holo_hash::AnyLinkableHash;

impl DbWrite<Dht> {
    /// Insert a row into the `Link` index table. Returns the number of rows inserted.
    pub async fn insert_link_index(&self, link: InsertLink<'_>) -> sqlx::Result<u64> {
        link::insert_link_index(self.pool(), link).await
    }
}

impl DbRead<Dht> {
    pub async fn get_links_by_base(&self, base: AnyLinkableHash) -> sqlx::Result<Vec<LinkRow>> {
        link::get_links_by_base(self.pool(), base).await
    }
}
