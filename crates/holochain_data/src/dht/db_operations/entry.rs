//! `DbRead<Dht>` / `DbWrite<Dht>` API for the `Entry` and `PrivateEntry` tables.

use super::super::inner::entry;
use crate::handles::{DbRead, DbWrite};
use crate::kind::Dht;
use holo_hash::{AgentPubKey, EntryHash};
use holochain_integrity_types::entry::Entry;

impl DbWrite<Dht> {
    pub async fn insert_entry(&self, hash: &EntryHash, entry: &Entry) -> sqlx::Result<()> {
        entry::insert_entry(self.pool(), hash, entry).await
    }

    pub async fn insert_private_entry(
        &self,
        hash: &EntryHash,
        author: &AgentPubKey,
        entry: &Entry,
    ) -> sqlx::Result<()> {
        entry::insert_private_entry(self.pool(), hash, author, entry).await
    }
}

impl DbRead<Dht> {
    /// Reads an entry by hash. Pass `author = Some(_)` to also surface a
    /// matching `PrivateEntry` owned by that author.
    pub async fn get_entry(
        &self,
        hash: EntryHash,
        author: Option<&AgentPubKey>,
    ) -> sqlx::Result<Option<Entry>> {
        entry::get_entry(self.pool(), hash, author).await
    }
}
