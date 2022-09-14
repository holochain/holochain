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
    /// The order of the integrity zomes.
    pub integrity_order: Vec<&'static str>,
    /// The set of inline zomes that will be installed as the coordinator zomes.
    pub coordinator_zomes: HashMap<&'static str, InlineCoordinatorZome>,
    /// The integrity zome dependencies for coordinator zomes.
    /// This is not needed if there is only a single integrity zome.
    pub dependencies: HashMap<ZomeName, ZomeName>,
}

#[allow(missing_docs)]
/// Some blank entry types to use for testing.
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

impl From<InlineEntryTypes> for ZomeEntryTypesKey {
    fn from(t: InlineEntryTypes) -> Self {
        Self {
            zome_index: 0.into(),
            type_index: (t as u8).into(),
        }
    }
}

impl InlineZomeSet {
    /// Create a set of integrity and coordinators zomes.
    pub fn new<I, C>(integrity: I, coordinators: C) -> Self
    where
        I: IntoIterator<Item = (&'static str, String, Vec<EntryDef>, u8)>,
        C: IntoIterator<Item = (&'static str, String)>,
    {
        let integrity_zomes: Vec<_> = integrity
            .into_iter()
            .map(|(zome_name, uuid, e, links)| {
                (zome_name, InlineIntegrityZome::new(uuid, e, links))
            })
            .collect();
        let integrity_order: Vec<_> = integrity_zomes.iter().map(|(n, _)| *n).collect();
        assert_eq!(integrity_order.len(), integrity_zomes.len());
        Self {
            integrity_zomes: integrity_zomes.into_iter().collect(),
            integrity_order,
            coordinator_zomes: coordinators
                .into_iter()
                .map(|(zome_name, uuid)| (zome_name, InlineCoordinatorZome::new(uuid)))
                .collect(),
            ..Default::default()
        }
    }

    /// Create a unique set of integrity and coordinators zomes.
    pub fn new_unique<I, C>(integrity: I, coordinators: C) -> Self
    where
        I: IntoIterator<Item = (&'static str, Vec<EntryDef>, u8)>,
        C: IntoIterator<Item = &'static str>,
    {
        let integrity_zomes: Vec<_> = integrity
            .into_iter()
            .map(|(zome_name, e, links)| (zome_name, InlineIntegrityZome::new_unique(e, links)))
            .collect();
        let integrity_order: Vec<_> = integrity_zomes.iter().map(|(n, _)| *n).collect();
        assert_eq!(integrity_order.len(), integrity_zomes.len());
        Self {
            integrity_zomes: integrity_zomes.into_iter().collect(),
            integrity_order,
            coordinator_zomes: coordinators
                .into_iter()
                .map(|zome_name| (zome_name, InlineCoordinatorZome::new_unique()))
                .collect(),
            ..Default::default()
        }
    }

    /// A helper function to create a single integrity and coordinator zome.
    pub fn new_single(
        integrity_zome_name: &'static str,
        coordinator_zome_name: &'static str,
        integrity_uuid: impl Into<String>,
        coordinator_uuid: impl Into<String>,
        entry_defs: Vec<EntryDef>,
        num_link_types: u8,
    ) -> Self {
        Self::new(
            [(
                integrity_zome_name,
                integrity_uuid.into(),
                entry_defs,
                num_link_types,
            )],
            [(coordinator_zome_name, coordinator_uuid.into())],
        )
    }

    /// A helper function to create a unique single integrity and coordinator zome.
    pub fn new_unique_single(
        integrity_zome_name: &'static str,
        coordinator_zome_name: &'static str,
        entry_defs: Vec<EntryDef>,
        num_link_types: u8,
    ) -> Self {
        Self::new_unique(
            [(integrity_zome_name, entry_defs, num_link_types)],
            [coordinator_zome_name],
        )
    }

    /// Add a callback to a zome with the given name.
    ///
    /// # Panics
    ///
    /// Panics if the zome_name doesn't exist for a zome in either set.
    pub fn function<F, I, O>(self, zome_name: &'static str, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        let Self {
            mut integrity_zomes,
            mut coordinator_zomes,
            dependencies,
            integrity_order,
        } = self;

        match integrity_zomes.remove_entry(zome_name) {
            Some((k, v)) => {
                integrity_zomes.insert(k, v.function(name, f));
            }
            None => {
                let (k, v) = coordinator_zomes.remove_entry(zome_name).unwrap();
                coordinator_zomes.insert(k, v.function(name, f));
            }
        }

        Self {
            integrity_zomes,
            integrity_order,
            coordinator_zomes,
            dependencies,
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
        self.integrity_order.extend(other.integrity_order);
        self.dependencies.extend(other.dependencies);
        self
    }

    /// Get the inner zomes
    pub fn into_zomes(mut self) -> (Vec<IntegrityZome>, Vec<CoordinatorZome>) {
        (
            self.integrity_zomes
                .into_iter()
                .map(|(n, z)| IntegrityZome::new((*n).into(), z.into()))
                .collect(),
            self.coordinator_zomes
                .into_iter()
                .map(|(n, z)| {
                    let mut z = CoordinatorZome::new((*n).into(), z.into());
                    let dep = self.dependencies.remove(z.zome_name());
                    if let Some(dep) = dep {
                        z.set_dependency(dep);
                    }
                    z
                })
                .collect(),
        )
    }

    /// Add a integrity dependency for a coordinator zome
    pub fn with_dependency(mut self, from: &'static str, to: &'static str) -> Self {
        assert!(
            self.coordinator_zomes.contains_key(from),
            "{} -> {}",
            to,
            from
        );
        assert!(self.integrity_zomes.contains_key(to), "{} -> {}", to, from);
        self.dependencies.insert(from.into(), to.into());
        self
    }

    /// Get the entry def location for committing an entry.
    pub fn get_entry_location(
        api: &BoxApi,
        index: impl Into<ZomeEntryTypesKey>,
    ) -> EntryDefLocation {
        let scoped_type = api
            .zome_info(())
            .unwrap()
            .zome_types
            .entries
            .get(index)
            .unwrap();
        EntryDefLocation::App(AppEntryDefLocation {
            zome_id: scoped_type.zome_id,
            entry_def_index: scoped_type.zome_type,
        })
    }

    /// Generate a link type filter from a link type.
    pub fn get_link_filter(api: &BoxApi, index: impl Into<ZomeLinkTypesKey>) -> LinkTypeFilter {
        let scoped_type = api
            .zome_info(())
            .unwrap()
            .zome_types
            .links
            .get(index)
            .unwrap();
        LinkTypeFilter::single_type(scoped_type.zome_id, scoped_type.zome_type)
    }

    /// Generate a link type filter for all dependencies of the this zome.
    pub fn dep_link_filter(api: &BoxApi) -> LinkTypeFilter {
        let zome_ids = api
            .zome_info(())
            .unwrap()
            .zome_types
            .links
            .dependencies()
            .collect();
        LinkTypeFilter::Dependencies(zome_ids)
    }
}

impl From<(&'static str, InlineIntegrityZome)> for InlineZomeSet {
    fn from((z, e): (&'static str, InlineIntegrityZome)) -> Self {
        let mut integrity_zomes = HashMap::new();
        integrity_zomes.insert(z, e);
        let integrity_order = vec![z];
        Self {
            integrity_zomes,
            integrity_order,
            ..Default::default()
        }
    }
}
