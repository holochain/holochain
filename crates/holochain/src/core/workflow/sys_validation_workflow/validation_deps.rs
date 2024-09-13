use holo_hash::HoloHash;
use holochain_cascade::CascadeSource;
use holochain_types::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone)]
/// The sources of all dependencies needed in sys validation.
/// Currently this comprises only action hashes within the same DHT, but could
/// some day include items from other DHTs or other sources.
pub struct SysValDeps {
    /// Dependencies found in the same DHT as the dependent
    pub same_dht: Arc<parking_lot::Mutex<ValidationDependencies<SignedActionHashed>>>,
    /// Dependencies found in the DPKI service's Deepkey DNA
    /// (this is not generic for all possible DPKI implementations, only Deepkey)
    /// (this is also currently unused! It may be useful if we need to be more explicit
    /// about missing DPKI dependencies, but so far it hasn't been a problem. If it's never
    /// a problem, this can be removed.)
    pub _deepkey_dht: Arc<parking_lot::Mutex<ValidationDependencies<EntryHashed>>>,
}

impl Default for SysValDeps {
    fn default() -> Self {
        Self {
            same_dht: Arc::new(parking_lot::Mutex::new(ValidationDependencies::new())),
            _deepkey_dht: Arc::new(parking_lot::Mutex::new(ValidationDependencies::new())),
        }
    }
}

/// A collection of validation dependencies for the current set of DHT ops requiring validation.
/// This is used as an in-memory cache of dependency info, held across all validation workflow calls,
/// to minimize the amount of network and database calls needed to check if dependencies have been satisfied
pub struct ValidationDependencies<T: HasHash = SignedActionHashed> {
    /// The state of each dependency, keyed by its hash.
    states: HashMap<HoloHash<T::HashType>, ValidationDependencyState<T>>,
    /// Tracks which dependencies have been accessed during a search for dependencies. Anything which
    /// isn't in this set is no longer needed for validation and can be dropped from [`states`].
    retained_deps: HashSet<HoloHash<T::HashType>>,
}

impl<T> Default for ValidationDependencies<T>
where
    T: holo_hash::HasHash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ValidationDependencies<T>
where
    T: holo_hash::HasHash,
{
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            retained_deps: HashSet::new(),
        }
    }

    /// Check whether a given dependency is currently held.
    /// Note that we may have this dependency as a key but the state won't contain the dependency because
    /// this is how we're tracking ops we know we need to fetch from the network.
    pub fn has(&mut self, hash: &HoloHash<T::HashType>) -> bool {
        self.retained_deps.insert(hash.clone());
        self.states
            .get(hash)
            .map(|state| state.dependency.is_some())
            .unwrap_or(false)
    }

    /// Get the state of a given dependency. This should always return a value because we should know about the dependency
    /// by examining the ops that are being validated. However, the dependency may not be found on the DHT yet.
    pub fn get(&self, hash: &HoloHash<T::HashType>) -> Option<&ValidationDependencyState<T>> {
        match self.states.get(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    pub fn get_mut(
        &mut self,
        hash: &HoloHash<T::HashType>,
    ) -> Option<&mut ValidationDependencyState<T>> {
        match self.states.get_mut(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    /// Get the hashes of all dependencies that are currently missing from the DHT.
    pub fn get_missing_hashes(&self) -> Vec<HoloHash<T::HashType>> {
        self.states
            .iter()
            .filter_map(|(hash, state)| {
                if state.dependency.is_none() {
                    Some(hash.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the hashes of all dependencies that have been fetched from the network.
    /// We need to let the incoming dht ops workflow know about these so that it can ingest them and get them validated.
    pub fn get_network_fetched_hashes(&self) -> Vec<HoloHash<T::HashType>> {
        self.states
            .iter()
            .filter_map(|(hash, state)| match state {
                ValidationDependencyState {
                    dependency:
                        Some(ValidationDependency {
                            fetched_from: CascadeSource::Network,
                            ..
                        }),
                    ..
                } => Some(hash.clone()),
                _ => None,
            })
            .collect()
    }

    /// Insert a record which was found after this set of dependencies was created.
    pub fn insert(&mut self, action: T, source: CascadeSource) -> bool {
        let hash = action.as_hash();

        // Note that `has` is checking that the dependency is actually set, not just that we have the key!
        if self.has(hash) {
            tracing::warn!(hash = ?hash, "Attempted to insert a dependency that was already present, this is not expected");
            return false;
        }

        self.retained_deps.insert(hash.clone());

        if let Some(s) = self.states.get_mut(hash) {
            s.set_dep(action);
            s.set_source(source);
            return true;
        }

        false
    }

    /// Forget which dependencies have been accessed since this method was last called.
    /// This is intended to be used with [`Self::purge_held_deps`] to remove any dependencies that are no longer needed.
    pub fn clear_retained_deps(&mut self) {
        self.retained_deps.clear();
    }

    /// Remove any dependencies that are no longer needed for validation.
    pub fn purge_held_deps(&mut self) {
        self.states.retain(|k, _| self.retained_deps.contains(k));
    }

    /// Merge the dependencies from another set into this one.
    pub fn merge(&mut self, other: Self) {
        self.retained_deps.extend(other.states.keys().cloned());
        self.states.extend(other.states);
    }

    pub fn new_from_iter<
        I: IntoIterator<Item = (HoloHash<T::HashType>, ValidationDependencyState<T>)>,
    >(
        iter: I,
    ) -> Self {
        Self {
            states: iter.into_iter().collect(),
            retained_deps: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValidationDependencyState<T> {
    /// The dependency if we've been able to fetch it, otherwise None until we manage to find it.
    dependency: Option<ValidationDependency<T>>,
}

impl<T> ValidationDependencyState<T> {
    pub fn new(dependency: Option<ValidationDependency<T>>) -> Self {
        Self { dependency }
    }

    pub fn single(dep: T, fetched_from: CascadeSource) -> Self {
        Self {
            dependency: Some(ValidationDependency { dep, fetched_from }),
        }
    }

    pub fn set_dep(&mut self, dep: T) {
        match self.dependency {
            None => {
                self.dependency = Some(ValidationDependency {
                    dep,
                    fetched_from: CascadeSource::Network,
                });
            }
            _ => {
                tracing::warn!("Attempted to set a record on a dependency that already has a value, this is a bug")
            }
        }
    }

    pub fn set_source(&mut self, new_source: CascadeSource) {
        match &mut self.dependency {
            Some(ValidationDependency { fetched_from, .. }) => {
                *fetched_from = new_source;
            }
            None => {}
        }
    }
}

impl ValidationDependencyState<SignedActionHashed> {
    pub fn as_action(&self) -> Option<&Action> {
        self.dependency.as_ref().map(|d| d.dep.action())
    }
}

/// A validation dependency which is either an Action or a Record, and the source of the dependency.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub struct ValidationDependency<T> {
    dep: T,
    fetched_from: CascadeSource,
}
