//! # Entry Defs Store
//! Stores all the entry definitions across zomes
use holochain_types::dna::DnaFile;
use holochain_zome_types::{entry_def::EntryDef, zome::ZomeName};
use holochain_state::{transaction::{Writer, Reader}, buffer::KvvBuf, prelude::{BufferedStore, MultiStore}, error::{DatabaseError, DatabaseResult}};

pub(crate) async fn get_entry_defs(dna: &DnaFile) -> Vec<(ZomeName, EntryDef)> {
    todo!()
}

pub struct EntryDefStore<'env>(KvvBuf<'env, ZomeName, EntryDef, Reader<'env>>);

/// This is where entry defs live
pub struct EntryDefBuf<'env>(EntryDefStore<'env>);

impl<'env> EntryDefBuf<'env> {
    pub fn new(reader: &'env Reader<'env>, entry_def_store: MultiStore) -> DatabaseResult<Self> {
        Ok(Self(KvvBuf::new(reader, entry_def_store)?))
    }

    // pub async fn get(&self, wasm_hash: &WasmHash) -> DatabaseResult<Option<DnaWasmHashed>> {
    //     self.0.get(&wasm_hash).await
    // }

    pub fn put(&mut self, k: ZomeName, v: Vec<EntryDef>) {
        self.0.put(k, v);
    }
}

impl<'env> BufferedStore<'env> for EntryDefBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}