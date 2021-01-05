//! # Entry Defs Store
//! Stores all the entry definitions across zomes
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::RibosomeT;

use super::api::CellConductorApiT;
use error::EntryDefStoreError;
use error::EntryDefStoreResult;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_lmdb::buffer::KvBufFresh;
use holochain_lmdb::error::DatabaseError;
use holochain_lmdb::error::DatabaseResult;
use holochain_lmdb::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_serialized_bytes::SerializedBytes;
use holochain_types::prelude::*;
use std::collections::HashMap;
use std::convert::TryInto;

pub mod error;

/// Key for the [EntryDef] buffer
#[derive(
    Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize, SerializedBytes,
)]
pub struct EntryDefBufferKey {
    zome: ZomeDef,
    entry_def_position: EntryDefIndex,
}

/// This is where entry defs live
pub struct EntryDefBuf(KvBufFresh<EntryDefStoreKey, EntryDef>);

#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
struct EntryDefStoreKey(SerializedBytes);

impl AsRef<[u8]> for EntryDefStoreKey {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl BufKey for EntryDefStoreKey {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self(UnsafeBytes::from(bytes.to_vec()).into())
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
    pub fn new(zome: ZomeDef, entry_def_position: EntryDefIndex) -> Self {
        Self {
            zome,
            entry_def_position,
        }
    }
}

impl EntryDefBuf {
    /// Create a new buffer
    pub fn new(env: EnvironmentRead, entry_def_store: SingleStore) -> DatabaseResult<Self> {
        Ok(Self(KvBufFresh::new(env, entry_def_store)))
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
    pub fn get_all<'r, R: Readable>(
        &self,
        r: &'r R,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = (EntryDefBufferKey, EntryDef), Error = DatabaseError> + 'r>,
    > {
        Ok(Box::new(
            self.0
                .store()
                .iter(r)?
                .map(|(k, v)| Ok((EntryDefStoreKey::from(k).into(), v))),
        ))
    }
}

impl BufferedStore for EntryDefBuf {
    type Error = DatabaseError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.0.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

/// Get an [EntryDef] from the entry def store
/// or fallback to running the zome
pub(crate) async fn get_entry_def(
    entry_def_index: EntryDefIndex,
    zome: ZomeDef,
    dna_def: &DnaDefHashed,
    conductor_api: &impl CellConductorApiT,
) -> EntryDefStoreResult<Option<EntryDef>> {
    // Try to get the entry def from the entry def store
    let key = EntryDefBufferKey::new(zome, entry_def_index);
    let entry_def = conductor_api.get_entry_def(&key).await;
    let dna_hash = dna_def.as_hash();
    let dna_file = conductor_api
        .get_dna(dna_hash)
        .await
        .ok_or_else(|| EntryDefStoreError::DnaFileMissing(dna_hash.clone()))?;

    // If it's not found run the ribosome and get the entry defs
    match &entry_def {
        Some(_) => Ok(entry_def),
        None => Ok(get_entry_defs(dna_file)?
            .get(entry_def_index.index())
            .map(|(_, v)| v.clone())),
    }
}

pub(crate) async fn get_entry_def_from_ids(
    zome_id: ZomeId,
    entry_def_index: EntryDefIndex,
    dna_def: &DnaDefHashed,
    conductor_api: &impl CellConductorApiT,
) -> EntryDefStoreResult<Option<EntryDef>> {
    match dna_def.zomes.get(zome_id.index()) {
        Some((_, zome)) => {
            get_entry_def(entry_def_index, zome.clone(), dna_def, conductor_api).await
        }
        None => Ok(None),
    }
}

#[tracing::instrument(skip(dna))]
/// Get all the [EntryDef] for this dna
pub(crate) fn get_entry_defs(
    dna: DnaFile, // TODO: make generic
) -> EntryDefStoreResult<Vec<(EntryDefBufferKey, EntryDef)>> {
    let invocation = EntryDefsInvocation;

    // Get the zomes hashes
    let zomes = dna
        .dna
        .zomes
        .iter()
        .cloned()
        .map(|(zome_name, zome)| (zome_name, zome))
        .collect::<HashMap<_, _>>();

    let ribosome = RealRibosome::new(dna);
    match ribosome.run_entry_defs(EntryDefsHostAccess, invocation)? {
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
                            let s = tracing::debug_span!("entry_def");
                            let _g = s.enter();
                            tracing::debug!(?entry_def);
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
            Err(EntryDefStoreError::CallbackFailed(zome_name, msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EntryDefBufferKey;
    use crate::conductor::Conductor;
    use holo_hash::HasHash;
    use holochain_lmdb::test_utils::test_environments;
    use holochain_types::dna::wasm::DnaWasmHashed;
    use holochain_types::dna::zome::ZomeDef;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn test_store_entry_defs() {
        observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let envs = test_environments();
        let handle = Conductor::builder().test(&envs).await.unwrap();

        let dna = fake_dna_zomes(
            "",
            vec![(TestWasm::EntryDefs.into(), TestWasm::EntryDefs.into())],
        );

        // Get expected entry defs
        let post_def = EntryDef {
            id: "post".into(),
            visibility: EntryVisibility::Public,
            crdt_type: CrdtType,
            required_validations: 5.into(),
            required_validation_type: Default::default(),
        };
        let comment_def = EntryDef {
            id: "comment".into(),
            visibility: EntryVisibility::Private,
            crdt_type: CrdtType,
            required_validations: 5.into(),
            required_validation_type: Default::default(),
        };
        let dna_wasm = DnaWasmHashed::from_content(TestWasm::EntryDefs.into())
            .await
            .into_hash();

        let post_def_key = EntryDefBufferKey {
            zome: ZomeDef::from_hash(dna_wasm.clone()),
            entry_def_position: 0.into(),
        };
        let comment_def_key = EntryDefBufferKey {
            zome: ZomeDef::from_hash(dna_wasm),
            entry_def_position: 1.into(),
        };

        handle.install_dna(dna).await.unwrap();
        // Check entry defs are here
        assert_eq!(
            handle.get_entry_def(&post_def_key).await,
            Some(post_def.clone())
        );
        assert_eq!(
            handle.get_entry_def(&comment_def_key).await,
            Some(comment_def.clone())
        );

        std::mem::drop(handle);

        // Restart conductor and check defs are still here
        let handle = Conductor::builder().test(&envs).await.unwrap();

        assert_eq!(handle.get_entry_def(&post_def_key).await, Some(post_def));
        assert_eq!(
            handle.get_entry_def(&comment_def_key).await,
            Some(comment_def.clone())
        );
    }
}
