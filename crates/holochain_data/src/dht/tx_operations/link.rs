//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `Link` table.

use super::super::inner::link::{self, InsertLink};
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::LinkRow;
use holo_hash::AnyLinkableHash;

impl TxWrite<Dht> {
    /// Insert a row into the `Link` index table. Returns the number of rows inserted.
    pub async fn insert_link_index(&mut self, link: InsertLink<'_>) -> sqlx::Result<u64> {
        link::insert_link_index(self.conn_mut(), link).await
    }
}

impl TxRead<Dht> {
    pub async fn get_links_by_base(&mut self, base: AnyLinkableHash) -> sqlx::Result<Vec<LinkRow>> {
        link::get_links_by_base(self.conn_mut(), base).await
    }
}
