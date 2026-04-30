//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `Link` table.

use super::super::inner::link;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use crate::models::dht::LinkRow;
use holo_hash::{ActionHash, AnyLinkableHash};

impl TxWrite<Dht> {
    pub async fn insert_link_index(
        &mut self,
        action_hash: &ActionHash,
        base_hash: &AnyLinkableHash,
        zome_index: u8,
        link_type: u8,
        tag: Option<&[u8]>,
    ) -> sqlx::Result<()> {
        link::insert_link_index(
            self.conn_mut(),
            action_hash,
            base_hash,
            zome_index,
            link_type,
            tag,
        )
        .await
    }
}

impl TxRead<Dht> {
    pub async fn get_links_by_base(&mut self, base: AnyLinkableHash) -> sqlx::Result<Vec<LinkRow>> {
        link::get_links_by_base(self.conn_mut(), base).await
    }
}
