//! Extra types that help with inline zomes that are not needed in the wasm.
use std::collections::HashMap;

use holochain_zome_types::prelude::*;
use serde::de::DeserializeOwned;

#[derive(Default)]
/// A set of inline integrity and coordinator zomes.
pub struct InlineZomeSet {
    /// The set of inline zomes that will be installed as the integrity zomes.
    /// Only these affect the [`DnaHash`](holo_hash::DnaHash).
    pub integrity_zomes: HashMap<&'static str, InlineIntegrityZome>,
    /// The set of inline zomes that will be installed as the coordinator zomes.
    pub coordinator_zomes: HashMap<&'static str, InlineCoordinatorZome>,
}

#[allow(missing_docs)]
#[repr(u8)]
/// Some black entry types to use for testing.
pub enum InlineEntryTypes {
    A,
    B,
    C,
}

impl InlineEntryTypes {
    /// Create the entry defs for tese types.
    pub fn entry_defs() -> Vec<EntryDef> {
        vec![Default::default(); 3]
    }
}

impl From<InlineEntryTypes> for LocalZomeTypeId {
    fn from(t: InlineEntryTypes) -> Self {
        Self(t as u8)
    }
}

impl InlineZomeSet {
    /// Create a set of integrity and coordinators zomes.
    pub fn new<I, C>(integrity: I, coordinators: C) -> Self
    where
        I: IntoIterator<Item = (&'static str, String, Vec<EntryDef>)>,
        C: IntoIterator<Item = (&'static str, String)>,
    {
        Self {
            integrity_zomes: integrity
                .into_iter()
                .map(|(zome_name, uuid, e)| (zome_name, InlineIntegrityZome::new(uuid, e)))
                .collect(),
            coordinator_zomes: coordinators
                .into_iter()
                .map(|(zome_name, uuid)| (zome_name, InlineCoordinatorZome::new(uuid)))
                .collect(),
        }
    }

    /// Create a set of integrity and coordinators zomes.
    pub fn new_unique<I, C>(integrity: I, coordinators: C) -> Self
    where
        I: IntoIterator<Item = (&'static str, Vec<EntryDef>)>,
        C: IntoIterator<Item = &'static str>,
    {
        Self {
            integrity_zomes: integrity
                .into_iter()
                .map(|(zome_name, e)| (zome_name, InlineIntegrityZome::new_unique(e)))
                .collect(),
            coordinator_zomes: coordinators
                .into_iter()
                .map(|zome_name| (zome_name, InlineCoordinatorZome::new_unique()))
                .collect(),
        }
    }

    /// A helper function to create a single integrity and coordinator zome.
    pub fn new_single(
        integrity_zome_name: &'static str,
        coordinator_zome_name: &'static str,
        integrity_uuid: impl Into<String>,
        coordinator_uuid: impl Into<String>,
        entry_defs: Vec<EntryDef>,
    ) -> Self {
        Self::new(
            [(integrity_zome_name, integrity_uuid.into(), entry_defs)],
            [(coordinator_zome_name, coordinator_uuid.into())],
        )
    }

    /// A helper function to create a unique single integrity and coordinator zome.
    pub fn new_unique_single(
        integrity_zome_name: &'static str,
        coordinator_zome_name: &'static str,
        entry_defs: Vec<EntryDef>,
    ) -> Self {
        Self::new_unique([(integrity_zome_name, entry_defs)], [coordinator_zome_name])
    }

    /// Add a callback to a zome with the given name.
    ///
    /// # Panics
    ///
    /// Panics if the zome_name doesn't exist for a zome in either set.
    pub fn callback<F, I, O>(self, zome_name: &'static str, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        let Self {
            mut integrity_zomes,
            mut coordinator_zomes,
        } = self;

        match integrity_zomes.remove_entry(zome_name) {
            Some((k, v)) => {
                integrity_zomes.insert(k, v.callback(name, f));
            }
            None => {
                let (k, v) = coordinator_zomes.remove_entry(zome_name).unwrap();
                coordinator_zomes.insert(k, v.callback(name, f));
            }
        }

        Self {
            integrity_zomes,
            coordinator_zomes,
        }
    }

    /// Merge two inline zome sets together.
    ///
    /// # Panics
    ///
    /// Panics if zome names collide across sets.
    pub fn merge(mut self, other: Self) -> Self {
        for (k, v) in other.integrity_zomes {
            if self.integrity_zomes.insert(k, v).is_some() {
                panic!("InlineZomeSet contains duplicate key {} on merge.", k);
            }
        }
        for (k, v) in other.coordinator_zomes {
            if self.coordinator_zomes.insert(k, v).is_some() {
                panic!("InlineZomeSet contains duplicate key {} on merge.", k);
            }
        }
        self
    }

    /// Get the inner zomes
    pub fn into_zomes(self) -> (Vec<IntegrityZome>, Vec<CoordinatorZome>) {
        (
            self.integrity_zomes
                .into_iter()
                .map(|(n, z)| IntegrityZome::new((*n).into(), z.into()))
                .collect(),
            self.coordinator_zomes
                .into_iter()
                .map(|(n, z)| CoordinatorZome::new((*n).into(), z.into()))
                .collect(),
        )
    }

    /// Get the entry def location for committing an entry.
    pub fn get_entry_location(api: &BoxApi, index: impl Into<LocalZomeTypeId>) -> EntryDefLocation {
        let r: EntryDefIndex = api
            .zome_info(())
            .unwrap()
            .zome_types
            .entries
            .to_global_scope(index)
            .unwrap()
            .into();
        r.into()
    }
}

impl From<(&'static str, InlineIntegrityZome)> for InlineZomeSet {
    fn from((z, e): (&'static str, InlineIntegrityZome)) -> Self {
        let mut integrity_zomes = HashMap::new();
        integrity_zomes.insert(z, e);
        Self {
            integrity_zomes,
            ..Default::default()
        }
    }
}
