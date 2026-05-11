//! `TxRead<Dht>` / `TxWrite<Dht>` API for the `Entry` and `PrivateEntry` tables.

use super::super::inner::entry;
use crate::handles::{TxRead, TxWrite};
use crate::kind::Dht;
use holo_hash::{AgentPubKey, EntryHash};
use holochain_integrity_types::entry::Entry;

impl TxWrite<Dht> {
    pub async fn insert_entry(&mut self, hash: &EntryHash, entry: &Entry) -> sqlx::Result<()> {
        entry::insert_entry(self.conn_mut(), hash, entry).await
    }

    pub async fn insert_private_entry(
        &mut self,
        hash: &EntryHash,
        author: &AgentPubKey,
        entry: &Entry,
    ) -> sqlx::Result<()> {
        entry::insert_private_entry(self.conn_mut(), hash, author, entry).await
    }
}

impl TxRead<Dht> {
    /// Reads an entry by hash. Pass `author = Some(_)` to also surface a
    /// matching `PrivateEntry` owned by that author.
    pub async fn get_entry(
        &mut self,
        hash: EntryHash,
        author: Option<&AgentPubKey>,
    ) -> sqlx::Result<Option<Entry>> {
        entry::get_entry(self.conn_mut(), hash, author).await
    }
}
