//! Transaction-scoped operations for the Wasm database table.
//!
//! Provides [`TxRead`] and [`TxWrite`] impls for querying and mutating the Wasm database table.

use holo_hash::{AgentPubKey, WasmHash};
use holochain_types::prelude::{CellId, DnaDef, DnaWasmHashed, EntryDef};

use crate::handles::{TxRead, TxWrite};
use crate::kind::Wasm;

use super::{inner_writes, reads};

impl TxRead<Wasm> {
    /// Check if WASM bytecode exists in the database.
    pub async fn wasm_exists(&mut self, hash: &WasmHash) -> sqlx::Result<bool> {
        reads::wasm_exists(self.conn_mut(), hash).await
    }

    /// Get WASM bytecode by hash.
    pub async fn get_wasm(&mut self, hash: &WasmHash) -> sqlx::Result<Option<DnaWasmHashed>> {
        reads::get_wasm(self.conn_mut(), hash).await
    }

    /// Check if a DNA definition exists in the database.
    pub async fn dna_def_exists(&mut self, cell_id: &CellId) -> sqlx::Result<bool> {
        reads::dna_def_exists(self.conn_mut(), cell_id).await
    }

    /// Get a DNA definition for the passed [`CellId`].
    pub async fn get_dna_def(&mut self, cell_id: &CellId) -> sqlx::Result<Option<DnaDef>> {
        reads::get_dna_def(self.tx_mut(), cell_id).await
    }

    /// Check if an entry definition exists in the database.
    pub async fn entry_def_exists(&mut self, key: &[u8]) -> sqlx::Result<bool> {
        reads::entry_def_exists(self.conn_mut(), key).await
    }

    /// Get an entry definition by key.
    pub async fn get_entry_def(&mut self, key: &[u8]) -> sqlx::Result<Option<EntryDef>> {
        reads::get_entry_def(self.conn_mut(), key).await
    }

    /// Get all entry definitions.
    pub async fn get_all_entry_defs(&mut self) -> sqlx::Result<Vec<(Vec<u8>, EntryDef)>> {
        reads::get_all_entry_defs(self.conn_mut()).await
    }

    /// Get all DNA definitions with their associated cell IDs.
    pub async fn get_all_dna_defs(&mut self) -> sqlx::Result<Vec<(CellId, DnaDef)>> {
        reads::get_all_dna_defs(self.tx_mut()).await
    }
}

impl TxWrite<Wasm> {
    /// Store WASM bytecode.
    pub async fn put_wasm(&mut self, wasm: DnaWasmHashed) -> sqlx::Result<()> {
        inner_writes::put_wasm(self.conn_mut(), wasm).await
    }

    /// Store a DNA definition and its associated zomes.
    ///
    /// Within a [`TxWrite`], this runs as a SAVEPOINT nested inside the
    /// outer transaction — it is atomic with the rest of the transaction.
    pub async fn put_dna_def(&mut self, agent: &AgentPubKey, dna_def: &DnaDef) -> sqlx::Result<()> {
        inner_writes::put_dna_def(self.tx_mut(), agent, dna_def).await
    }

    /// Store an entry definition.
    pub async fn put_entry_def(&mut self, key: Vec<u8>, entry_def: &EntryDef) -> sqlx::Result<()> {
        inner_writes::put_entry_def(self.conn_mut(), key, entry_def).await
    }
}
