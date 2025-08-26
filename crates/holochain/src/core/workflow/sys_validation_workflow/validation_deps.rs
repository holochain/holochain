use holochain_cascade::CascadeSource;
use holochain_types::prelude::*;
use std::ops::Deref;
use std::sync::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// The sources of all dependencies needed in sys validation.
#[derive(Clone)]
pub struct SysValDeps {
    /// Dependencies found in the same DHT as the dependent
    validation_dependencies: Arc<Mutex<ValidationDependencies>>,
}

impl Default for SysValDeps {
    fn default() -> Self {
        Self {
            validation_dependencies: Arc::new(Mutex::new(ValidationDependencies::new())),
        }
    }
}

impl Deref for SysValDeps {
    type Target = Arc<Mutex<ValidationDependencies>>;

    fn deref(&self) -> &Self::Target {
        &self.validation_dependencies
    }
}

/// A collection of validation dependencies for the current set of DHT ops requiring validation.
///
/// This is used as an in-memory cache of dependency info, held across all validation workflow calls,
/// to minimize the number of network and database calls needed to check if dependencies have been
/// satisfied.
pub struct ValidationDependencies {
    /// The state of each dependency, keyed by its hash.
    states: HashMap<ActionHash, ValidationDependencyState>,
    /// Tracks which dependencies have been accessed during a search for dependencies. Anything not
    /// in this set is no longer needed for validation and can be dropped from [`states`].
    retained_deps: HashSet<ActionHash>,
}

impl Default for ValidationDependencies
{
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationDependencies
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
    pub fn has(&mut self, hash: &ActionHash) -> bool {
        self.retained_deps.insert(hash.clone());
        self.states
            .get(hash)
            .map(|state| state.dependency.is_some())
            .unwrap_or(false)
    }

    /// Get the state of a given dependency. This should always return a value because we should know about the dependency
    /// by examining the ops that are being validated. However, the dependency may not be found on the DHT yet.
    pub fn get(&self, hash: &ActionHash) -> Option<&ValidationDependencyState> {
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
        hash: &ActionHash,
    ) -> Option<&mut ValidationDependencyState> {
        match self.states.get_mut(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    /// Get the hashes of all dependencies that are currently missing from the DHT.
    pub fn get_missing_hashes(&self) -> Vec<ActionHash> {
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
    pub fn get_network_fetched_hashes(&self) -> Vec<ActionHash> {
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
    pub fn insert(&mut self, action: SignedActionHashed, source: CascadeSource) -> bool {
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
        I: IntoIterator<Item = (ActionHash, ValidationDependencyState)>,
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
pub struct ValidationDependencyState {
    /// The dependency if we've been able to fetch it, otherwise None until we manage to find it.
    dependency: Option<ValidationDependency>,
}

impl ValidationDependencyState {
    pub fn new(dependency: Option<ValidationDependency>) -> Self {
        Self { dependency }
    }

    pub fn single(dep: SignedActionHashed, fetched_from: CascadeSource) -> Self {
        Self {
            dependency: Some(ValidationDependency { dep, fetched_from }),
        }
    }

    pub fn set_dep(&mut self, dep: SignedActionHashed) {
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

    /// Set the source of the dependency.
    ///
    /// This is used to track where the dependency was found.
    pub fn set_source(&mut self, new_source: CascadeSource) {
        if let Some(ValidationDependency { fetched_from, .. }) = &mut self.dependency {
            *fetched_from = new_source;
        }
    }
}

impl ValidationDependencyState {
    /// Get the action from the dependency state if it is present.
    pub fn as_action(&self) -> Option<&Action> {
        self.dependency.as_ref().map(|d| d.dep.action())
    }
}

/// A validation dependency which is either an Action or a Record, and the source of the dependency.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub struct ValidationDependency {
    dep: SignedActionHashed,
    fetched_from: CascadeSource,
}
