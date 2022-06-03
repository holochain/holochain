//! # Entry Defs Store
//! Stores all the entry definitions across zomes
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsInvocation;
use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::RibosomeT;

use error::EntryDefStoreError;
use error::EntryDefStoreResult;
use holo_hash::*;
use holochain_types::prelude::*;
use std::collections::HashMap;

use super::handle::ConductorHandleT;

pub mod error;

/// Get an [EntryDef] from the entry def store
/// or fallback to running the zome
pub(crate) async fn get_entry_def(
    entry_def_index: EntryDefIndex,
    zome: IntegrityZomeDef,
    dna_hash: &DnaHash,
    conductor_handle: &dyn ConductorHandleT,
) -> EntryDefStoreResult<Option<EntryDef>> {
    // Try to get the entry def from the entry def store
    let key = EntryDefBufferKey::new(zome, entry_def_index);
    let entry_def = conductor_handle.get_entry_def(&key);
    let ribosome = conductor_handle
        .get_ribosome(dna_hash)
        .map_err(|_| EntryDefStoreError::DnaFileMissing(dna_hash.clone()))?;

    // If it's not found run the ribosome and get the entry defs
    match &entry_def {
        Some(_) => Ok(entry_def),
        None => Ok(get_entry_defs(ribosome)
            .await?
            .into_iter()
            .find(
                |(
                    EntryDefBufferKey {
                        entry_def_position, ..
                    },
                    _,
                )| *entry_def_position == entry_def_index,
            )
            .map(|(_, v)| v)),
    }
}

pub(crate) async fn get_entry_def_from_ids(
    entry_def_index: EntryDefIndex,
    dna_hash: &DnaHash,
    conductor_handle: &dyn ConductorHandleT,
) -> EntryDefStoreResult<Option<EntryDef>> {
    let ribosome = conductor_handle
        .get_ribosome(dna_hash)
        .map_err(|_| EntryDefStoreError::DnaFileMissing(dna_hash.clone()))?;
    match ribosome.find_zome_from_entry(&entry_def_index) {
        Some(zome) => {
            get_entry_def(
                entry_def_index,
                zome.into_inner().1,
                dna_hash,
                conductor_handle,
            )
            .await
        }
        None => Ok(None),
    }
}

#[tracing::instrument(skip(ribosome))]
/// Get all the [EntryDef] for this dna
pub(crate) async fn get_entry_defs(
    ribosome: RealRibosome,
) -> EntryDefStoreResult<Vec<(EntryDefBufferKey, EntryDef)>> {
    let invocation = EntryDefsInvocation;

    // Get the zomes hashes
    let zomes = ribosome
        .dna_def()
        .integrity_zomes
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, (zome_name, zome))| (zome_name, (ZomeId(i as u8), zome)))
        .collect::<HashMap<_, _>>();

    let (ribosome, result) = tokio::task::spawn_blocking(move || {
        let r = ribosome.run_entry_defs(EntryDefsHostAccess, invocation);
        (ribosome, r)
    })
    .await?;

    let zome_types_map = ribosome.zome_types();
    match result? {
        EntryDefsResult::Defs(map) => {
            // Turn the defs map into a vec of keys and entry defs
            map.into_iter()
                // Skip zomes without entry defs
                .filter_map(|(zome_name, entry_defs)| {
                    zomes.get(&zome_name).map(|zome| (zome.clone(), entry_defs))
                })
                // Get each entry def and pair with a key
                .flat_map(|((zome_id, zome), entry_defs)| {
                    entry_defs
                        .into_iter()
                        .enumerate()
                        .map(move |(local_index, entry_def)| {
                            let entry_def_position = u8::try_from(local_index)
                                .ok()
                                .and_then(|local_type_id| {
                                    zome_types_map
                                        .re_scope(&[zome_id])
                                        .ok()?
                                        .entries
                                        .to_global_scope(local_type_id)
                                })
                                .ok_or(EntryDefStoreError::EntryTypeMissing)?;

                            Ok((
                                EntryDefBufferKey {
                                    zome: zome.clone(),
                                    entry_def_position: entry_def_position.into(),
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
    use holochain_state::prelude::test_db_dir;
    use holochain_types::prelude::*;
    use holochain_types::test_utils::fake_dna_zomes;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_store_entry_defs() {
        observability::test_run().ok();

        // all the stuff needed to have a WasmBuf
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();

        let dna = fake_dna_zomes(
            "",
            vec![(TestWasm::EntryDefs.into(), TestWasm::EntryDefs.into())],
        );

        // Get expected entry defs
        let post_def = EntryDef {
            id: "post".into(),
            visibility: EntryVisibility::Public,
            required_validations: 5.into(),
            required_validation_type: Default::default(),
        };
        let comment_def = EntryDef {
            id: "comment".into(),
            visibility: EntryVisibility::Private,
            required_validations: 5.into(),
            required_validation_type: Default::default(),
        };
        let dna_wasm = DnaWasmHashed::from_content(TestWasm::EntryDefs.into())
            .await
            .into_hash();

        let post_def_key = EntryDefBufferKey {
            zome: IntegrityZomeDef::from_hash(dna_wasm.clone()),
            entry_def_position: 0.into(),
        };
        let comment_def_key = EntryDefBufferKey {
            zome: IntegrityZomeDef::from_hash(dna_wasm),
            entry_def_position: 1.into(),
        };

        handle.register_dna(dna).await.unwrap();
        // Check entry defs are here
        assert_eq!(handle.get_entry_def(&post_def_key), Some(post_def.clone()));
        assert_eq!(
            handle.get_entry_def(&comment_def_key),
            Some(comment_def.clone())
        );

        std::mem::drop(handle);

        // Restart conductor and check defs are still here
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();

        assert_eq!(handle.get_entry_def(&post_def_key), Some(post_def));
        assert_eq!(
            handle.get_entry_def(&comment_def_key),
            Some(comment_def.clone())
        );
    }
}
