use holo_hash::AnyDhtHash;
use holochain_cascade::CascadeSource;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::{
    action::ActionHashed,
    record::{Record, SignedActionHashed},
    Action,
};
use std::collections::{HashMap, HashSet};

/// A collection of validation dependencies for the current set of DHT ops requiring validation.
pub struct ValidationDependencies {
    /// The state of each dependency, keyed by its hash.
    states: HashMap<AnyDhtHash, ValidationDependencyState>,
    /// Tracks which dependencies have been accessed during a search for dependencies. Anything which
    /// isn't in this set is no longer needed for validation and can be dropped from [`states`].
    retained_deps: HashSet<AnyDhtHash>,
}

impl Default for ValidationDependencies {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationDependencies {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            retained_deps: HashSet::new(),
        }
    }

    /// Check whether a given dependency is currently held.
    /// Note that we may have this dependency as a key but the state won't contain the dependency because
    /// this is how we're tracking ops we know we need to fetch from the network.
    pub fn has(&mut self, hash: &AnyDhtHash) -> bool {
        self.retained_deps.insert(hash.clone());
        self.states
            .get(hash)
            .map(|state| state.dependency.is_some())
            .unwrap_or(false)
    }

    /// Get the state of a given dependency. This should always return a value because we should know about the depdendency
    /// by examining the ops that are being validated. However, the dependency may not be found on the DHT yet.
    pub fn get(&mut self, hash: &AnyDhtHash) -> Option<&mut ValidationDependencyState> {
        match self.states.get_mut(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    /// Get the hashes of all dependencies that are currently missing from the DHT.
    pub fn get_missing_hashes(&self) -> Vec<AnyDhtHash> {
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
    pub fn get_network_fetched_hashes(&self) -> Vec<AnyDhtHash> {
        self.states
            .iter()
            .filter_map(|(hash, state)| match state {
                ValidationDependencyState {
                    dependency: Some(ValidationDependency::Action(_, CascadeSource::Network)),
                    ..
                }
                | ValidationDependencyState {
                    dependency: Some(ValidationDependency::Record(_, CascadeSource::Network)),
                    ..
                } => Some(hash.clone()),
                _ => None,
            })
            .collect()
    }

    /// Insert a record which was found after this set of dependencies was created.
    pub fn insert(&mut self, record: Record, source: CascadeSource) -> bool {
        let hash: AnyDhtHash = ActionHashed::from_content_sync(record.action().clone())
            .hash
            .into();

        // Note that `has` is checking that the dependency is actually set, not just that we have the key!
        if self.has(&hash) {
            tracing::warn!(hash = ?hash, "Attempted to insert a dependency that was already present, this is not expected");
            return false;
        }

        self.retained_deps.insert(hash.clone());

        if let Some(s) = self.states.get_mut(&hash) {
            s.set_record(record);
            s.set_source(source);
            return true;
        }

        false
    }

    /// Forget which dependencies have been accessed since this method was last called.
    /// This is intended to be used with [`purge_held_deps`] to remove any dependencies that are no longer needed.
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
}

impl FromIterator<(AnyDhtHash, ValidationDependencyState)> for ValidationDependencies {
    fn from_iter<T: IntoIterator<Item = (AnyDhtHash, ValidationDependencyState)>>(iter: T) -> Self {
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
    /// The type of the op that referenced this dependency
    required_by_op_type: Option<DhtOpType>,
}

impl ValidationDependencyState {
    pub fn new(
        dependency: Option<ValidationDependency>,
        required_by_op_type: Option<DhtOpType>,
    ) -> Self {
        Self {
            dependency,
            required_by_op_type,
        }
    }

    pub fn required_by_op_type(&self) -> Option<DhtOpType> {
        self.required_by_op_type
    }

    pub fn set_record(&mut self, record: Record) {
        match &mut self.dependency {
            None => {
                self.dependency = Some(ValidationDependency::Record(record, CascadeSource::Network))
            }
            _ => {
                tracing::warn!("Attempted to set a record on a dependency that already has a value, this is a bug")
            }
        }
    }

    pub fn set_source(&mut self, new_source: CascadeSource) {
        match &mut self.dependency {
            Some(ValidationDependency::Action(_, source)) => {
                *source = new_source;
            }
            Some(ValidationDependency::Record(_, source)) => {
                *source = new_source;
            }
            None => {}
        }
    }

    pub fn as_action(&self) -> Option<&Action> {
        match &self.dependency {
            Some(ValidationDependency::Action(signed_action, _)) => Some(signed_action.action()),
            Some(ValidationDependency::Record(record, _)) => Some(record.action()),
            None => None,
        }
    }

    pub fn as_record(&self) -> Option<&Record> {
        match &self.dependency {
            Some(ValidationDependency::Action(_, _)) => {
                tracing::warn!(
                    "Attempted to get a record from a dependency that is an action, this is a bug"
                );
                None
            }
            Some(ValidationDependency::Record(record, _)) => Some(record),
            None => None,
        }
    }
}

/// A validation dependency which is either an Action or a Record, and the source of the dependency.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ValidationDependency {
    /// The dependency is represented by an Action when it has been fetched for inline validation.
    Action(SignedActionHashed, CascadeSource),
    /// The dependency is represented by a Record when it has been fetched by the sys validation workflow.
    /// Retaining the record allows us to build a DHT op which can then be sent to the incoming dht ops
    /// workflow when the dependency has been fetched from the network.
    Record(Record, CascadeSource),
}

impl From<(SignedActionHashed, CascadeSource)> for ValidationDependencyState {
    fn from((signed_action, fetched_from): (SignedActionHashed, CascadeSource)) -> Self {
        Self {
            dependency: Some(ValidationDependency::Action(signed_action, fetched_from)),
            required_by_op_type: None,
        }
    }
}

impl From<(Record, CascadeSource, DhtOpType)> for ValidationDependencyState {
    fn from((record, fetched_from, op_type): (Record, CascadeSource, DhtOpType)) -> Self {
        Self {
            dependency: Some(ValidationDependency::Record(record, fetched_from)),
            required_by_op_type: Some(op_type),
        }
    }
}
