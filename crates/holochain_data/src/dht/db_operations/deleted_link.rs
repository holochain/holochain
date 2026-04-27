//! `DbRead<Dht>` API for the `DeletedLink` table.
//!
//! `insert_deleted_link_index` is intentionally only on `TxWrite<Dht>` — see
//! [`super::super::tx_operations::deleted_link`].

use super::super::inner::deleted_link;
use crate::handles::DbRead;
use crate::kind::Dht;
use crate::models::dht::DeletedLinkRow;
use holo_hash::ActionHash;

impl DbRead<Dht> {
    pub async fn get_deleted_links(
        &self,
        create_link_hash: ActionHash,
    ) -> sqlx::Result<Vec<DeletedLinkRow>> {
        deleted_link::get_deleted_links(self.pool(), create_link_hash).await
    }
}
