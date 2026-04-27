//! `DbRead<Dht>` API for the `Link` table.
//!
//! `insert_link_index` is intentionally only on `TxWrite<Dht>` — see
//! [`super::super::tx_operations::link`].

use super::super::inner::link;
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::LinkRow;
use holo_hash::AnyLinkableHash;

impl DbRead<Dht> {
    pub async fn get_links_by_base(&self, base: AnyLinkableHash) -> sqlx::Result<Vec<LinkRow>> {
        link::get_links_by_base(self.pool(), base).await
    }
}
