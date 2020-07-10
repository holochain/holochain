//! # Entry Defs Store
//! Stores all the entry definitions across zomes
use crate::core::ribosome::{
    guest_callback::entry_defs::{EntryDefsInvocation, EntryDefsResult},
    wasm_ribosome::WasmRibosome,
    RibosomeT,
};
use derive_more::From;
use error::{EntryDefStoreError, EntryDefStoreResult};
use fallible_iterator::FallibleIterator;
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{
    buffer::KvBuf,
    error::{DatabaseError, DatabaseResult},
    prelude::{BufferedStore, SingleStore},
    transaction::{Reader, Writer},
};
use holochain_types::{
    dna::{zome::Zome, DnaFile},
    header::EntryDefId,
};
use holochain_zome_types::entry_def::EntryDef;
use shrinkwraprs::Shrinkwrap;
use std::{collections::HashMap, convert::TryInto};

pub mod error;

/// Key for the [EntryDef] buffer
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, SerializedBytes)]
pub struct EntryDefBufferKey {
    zome: Zome,
    entry_def_position: EntryDefId,
}

/// This is where entry defs live
pub struct EntryDefBuf<'env>(EntryDefStore<'env>);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct EntryDefStoreKey(SerializedBytes);

#[derive(From, Shrinkwrap)]
#[shrinkwrap(mutable)]
struct EntryDefStore<'env>(KvBuf<'env, EntryDefStoreKey, EntryDef, Reader<'env>>);

impl AsRef<[u8]> for EntryDefStoreKey {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl From<EntryDefBufferKey> for EntryDefStoreKey {
    fn from(a: EntryDefBufferKey) -> Self {
        Self(
            a.try_into()
                .expect("EntryDefStoreKey serialization cannot fail"),
        )
    }
}

impl From<&[u8]> for EntryDefStoreKey {
    fn from(bytes: &[u8]) -> Self {
        Self(UnsafeBytes::from(bytes.to_vec()).into())
    }
}

impl From<EntryDefStoreKey> for EntryDefBufferKey {
    fn from(a: EntryDefStoreKey) -> Self {
        a.0.try_into()
            .expect("Database corruption when retrieving EntryDefBufferKeys")
    }
}

impl EntryDefBufferKey {
    /// Create a new key
    pub fn new(zome: Zome, entry_def_position: EntryDefId) -> Self {
        Self {
            zome,
            entry_def_position,
        }
    }
}

impl<'env> EntryDefBuf<'env> {
    /// Create a new buffer
    pub fn new(reader: &'env Reader<'env>, entry_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self(KvBuf::new(reader, entry_def_store)?.into()))
    }

    /// Get an entry def
    pub fn get(&self, k: EntryDefBufferKey) -> DatabaseResult<Option<EntryDef>> {
        self.0.get(&k.into())
    }

    /// Store an entry def
    pub fn put(&mut self, k: EntryDefBufferKey, entry_def: EntryDef) -> DatabaseResult<()> {
        self.0.put(k.into(), entry_def)
    }

    /// Get all the entry defs in the database
    pub fn get_all(
        &self,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = (EntryDefBufferKey, EntryDef), Error = DatabaseError> + '_>,
    > {
        Ok(Box::new(
            self.0
                .iter_raw()?
                .map(|(k, v)| Ok((EntryDefStoreKey::from(k).into(), v))),
        ))
    }
}

impl<'env> BufferedStore<'env> for EntryDefBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        let store = self.0;
        store.0.flush_to_txn(writer)?;
        Ok(())
    }
}

/// Get all the [EntryDef] for this dna
pub(crate) async fn get_entry_defs(
    dna: DnaFile,
) -> EntryDefStoreResult<Vec<(EntryDefBufferKey, EntryDef)>> {
    let invocation = EntryDefsInvocation::new();
    // Get the zomes hashes
    let zomes = dna
        .dna
        .zomes
        .iter()
        .cloned()
        .map(|(zome_name, zome)| (zome_name, zome))
        .collect::<HashMap<_, _>>();

    let ribosome = WasmRibosome::new(dna);
    match ribosome.run_entry_defs(invocation)? {
        EntryDefsResult::Defs(map) => {
            // Turn the defs map into a vec of keys and entry defs
            map.into_iter()
                // Skip zomes without entry defs
                .filter_map(|(zome_name, entry_defs)| {
                    zomes.get(&zome_name).map(|zome| (zome.clone(), entry_defs))
                })
                // Get each entry def and pair with a key
                .flat_map(|(zome, entry_defs)| {
                    entry_defs
                        .into_iter()
                        .enumerate()
                        .map(move |(i, entry_def)| {
                            Ok((
                                EntryDefBufferKey {
                                    zome: zome.clone(),
                                    // Entry positions are stored as u8 so we can't have more then 255
                                    entry_def_position: u8::try_from(i)
                                        .map_err(|_| EntryDefStoreError::TooManyEntryDefs)?
                                        .into(),
                                },
                                entry_def,
                            ))
                        })
                })
                .collect()
        }
        EntryDefsResult::Err(zome_name, msg) => {
            return Err(EntryDefStoreError::CallbackFailed(zome_name, msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::conductor::Conductor;
    use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn test_store_entry_defs() {
        holochain_types::observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let test_env = test_conductor_env();
        let TestEnvironment {
            env: wasm_env,
            tmpdir: _tmpdir,
        } = test_wasm_env();
        let _tmpdir = test_env.tmpdir.clone();
        let handle = Conductor::builder().test(test_env, wasm_env).await.unwrap();
        let shutdown = handle.take_shutdown_handle().await.unwrap();

        let dna = fake_dna_zomes(
            "",
            vec![(TestWasm::EntryDefs.into(), TestWasm::EntryDefs.into())],
        );

        // TODO: Get expected entry defs

        handle.install_dna(dna).await.unwrap();
        // TODO: Check entry defs are here
        handle.shutdown().await;
        shutdown.await.unwrap();
        // TODO: Restart conductor and check defs are still here
    }
}
