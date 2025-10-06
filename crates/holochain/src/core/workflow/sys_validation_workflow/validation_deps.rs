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
    ///
    /// Note that we may have this dependency as a key but the state won't contain the dependency because
    /// this is how we're tracking ops we know we need to fetch from the network.
    pub fn has(&mut self, hash: &ActionHash) -> bool {
        self.retained_deps.insert(hash.clone());
        self.states
            .get(hash)
            .map(|state| state.dependency.is_some())
            .unwrap_or(false)
    }

    /// Get the state of a given dependency.
    ///
    /// This should always return a value because we should know about the dependency by examining
    /// the ops that are being validated. However, the dependency may not be found on the DHT yet.
    pub fn get(&self, hash: &ActionHash) -> Option<&ValidationDependencyState> {
        match self.states.get(hash) {
            Some(dep) => Some(dep),
            None => {
                tracing::warn!(hash = ?hash, "Have not attempted to fetch requested dependency, this is a bug");
                None
            }
        }
    }

    /// Get the hashes and types of all dependencies that are currently missing from the DHT.
    pub(super) fn get_missing_dependencies(&self) -> Vec<(ActionHash, ValidationDependencyType)> {
        self.states
            .iter()
            .filter_map(|(hash, state)| {
                if state.dependency.is_none() {
                    Some((hash.clone(), state.dependency_type.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Insert an action which was found after this set of dependencies was created.
    pub fn insert_action(&mut self, action: SignedActionHashed, source: CascadeSource) -> bool {
        let hash = action.as_hash();

        // Note that `has` is checking that the dependency is actually set, not just that we have the key!
        if self.has(hash) {
            tracing::warn!(hash = ?hash, "Attempted to insert a dependency that was already present, this is not expected");
            return false;
        }

        if let Some(s) = self.states.get_mut(hash) {
            s.set_action_dep(action);
            s.set_source(source);
            return true;
        }

        false
    }

    /// Insert a record which was found after this set of dependencies was created.
    pub fn insert_pending_validation_warranted(
        &mut self,
        action: SignedActionHashed,
        chain_op_type: ChainOpType,
        source: CascadeSource,
    ) -> bool {
        let hash = action.as_hash();

        // Note that `has` is checking that the dependency is actually set, not just that we have the key!
        if self.has(hash) {
            tracing::warn!(hash = ?hash, "Attempted to insert a dependency that was already present, this is not expected");
            return false;
        }

        if let Some(s) = self.states.get_mut(hash) {
            s.set_pending_validation_warranted(action, chain_op_type);
            s.set_source(source);
            return true;
        }

        false
    }

    /// Update a warranted dependency with its validation status.
    pub fn update_warrant_dep_validated(
        &mut self,
        action: &ActionHash,
        validation_status: ValidationStatus,
    ) {
        // Note that `has` is checking that the dependency is actually set, not just that we have the key!
        if !self.has(action) {
            tracing::warn!(hash = ?action, "Attempted to update a dependency that is not present, this is not expected");
            return;
        }

        if let Some(s) = self.states.get_mut(action) {
            s.set_validation_status_for_warranted(validation_status);
        }
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

        // When merging, prefer keeping warrant records because those contain actions too, but if
        // an action overwrites the record then we don't have the entry anymore.
        for (hash, other_state) in other.states {
            match self.states.entry(hash.clone()) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(other_state);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let existing = e.get_mut();
                    match (&existing.dependency, &other_state.dependency) {
                        (None, Some(_)) => {
                            // If we don't have it but the other does, take it.
                            *existing = other_state;
                        }
                        (Some(_), None) => {
                            // If we have it but the other doesn't, keep what we have.
                        }
                        (Some(_), Some(_)) => {
                            // If we both have it, prefer keeping a record dependency over an action
                            // dependency.
                            if existing.dependency_type == ValidationDependencyType::Action
                                && matches!(
                                    other_state.dependency_type,
                                    ValidationDependencyType::Warranted(_)
                                )
                            {
                                *existing = other_state;
                            }
                        }
                        (None, None) => {
                            // If neither side has a value, don't make changes.
                        }
                    }
                }
            }
        }
    }

    pub fn new_from_iter<I: IntoIterator<Item = (ActionHash, ValidationDependencyState)>>(
        iter: I,
    ) -> Self {
        Self {
            states: iter.into_iter().collect(),
            retained_deps: HashSet::new(),
        }
    }

    /// Get the list of warranted dependencies that are pending validation.
    pub fn get_pending_warrant_dependencies(&self) -> Vec<(ActionHash, ChainOpType)> {
        self.states
            .values()
            .filter_map(|state| match (&state.dependency_type, &state.dependency) {
                (ValidationDependencyType::Warranted(_), Some(v)) => match &v.value {
                    ValidationDependencyValue::Warranted(WarrantedDep::Pending(r, t)) => {
                        Some((r.action_address().clone(), *t))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct ValidationDependencyState {
    /// The type of dependency this is, either an Action or a Record.
    dependency_type: ValidationDependencyType,
    /// The dependency if we've been able to fetch it, otherwise None until we manage to find it.
    dependency: Option<ValidationDependency>,
}

impl ValidationDependencyState {
    pub(super) fn new_present(
        value: ValidationDependencyValue,
        fetched_from: CascadeSource,
    ) -> Self {
        Self {
            dependency_type: match value {
                ValidationDependencyValue::Action(_) => ValidationDependencyType::Action,
                ValidationDependencyValue::Warranted(WarrantedDep::Pending(_, chain_op_type)) => {
                    ValidationDependencyType::Warranted(chain_op_type)
                }
                ValidationDependencyValue::Warranted(WarrantedDep::Validated(_, _)) => {
                    unreachable!("Cannot create a new present dependency that already has a validated dependency")
                }
            },
            dependency: Some(ValidationDependency {
                value,
                fetched_from,
            }),
        }
    }

    pub(super) fn new_pending(dependency_type: ValidationDependencyType) -> Self {
        Self {
            dependency_type,
            dependency: None,
        }
    }

    /// Provide the missing [`SignedActionHashed`] for this action dependency.
    pub(super) fn set_action_dep(&mut self, action: SignedActionHashed) {
        if !matches!(self.dependency_type, ValidationDependencyType::Action) {
            tracing::warn!("Attempted to set an action dependency for a validation dependency that is not expecting an action, this is a bug");
            return;
        }

        match self.dependency {
            None => {
                self.dependency = Some(ValidationDependency {
                    value: ValidationDependencyValue::Action(action),
                    fetched_from: CascadeSource::Network,
                });
            }
            _ => {
                tracing::warn!(
                    "Attempted to set an action dependency that already has a value, this is a bug"
                )
            }
        }
    }

    /// Provide the missing [`SignedActionHashed`] for this warranted record dependency, marking it
    /// as pending validation.
    pub(super) fn set_pending_validation_warranted(
        &mut self,
        action: SignedActionHashed,
        op_type: ChainOpType,
    ) {
        if !matches!(self.dependency_type, ValidationDependencyType::Warranted(_)) {
            tracing::warn!("Attempted to set a warranted record dependency for a validation dependency that is not expecting Warranted, this is a bug");
            return;
        }

        match self.dependency {
            None => {
                self.dependency = Some(ValidationDependency {
                    value: ValidationDependencyValue::Warranted(WarrantedDep::Pending(
                        action, op_type,
                    )),
                    fetched_from: CascadeSource::Network,
                });
            }
            _ => {
                tracing::warn!("Attempted to set a warranted record dependency that already has a value, this is a bug")
            }
        }
    }

    /// Set the validation status for a warranted record dependency that has now been validated.
    pub(super) fn set_validation_status_for_warranted(
        &mut self,
        validation_status: ValidationStatus,
    ) {
        if !matches!(self.dependency_type, ValidationDependencyType::Warranted(_)) {
            tracing::warn!("Attempted to set an action on a dependency that is not of type Action, this is a bug");
            return;
        }

        if let Some(ValidationDependency { value, .. }) = self.dependency.as_mut() {
            match value {
                ValidationDependencyValue::Warranted(WarrantedDep::Pending(action, _)) => {
                    let action = action.clone();
                    *value = ValidationDependencyValue::Warranted(WarrantedDep::Validated(
                        action,
                        validation_status,
                    ));
                }
                _ => {
                    tracing::warn!("Attempted to set a status for a dependency that is not pending validation, this is a bug");
                }
            }
        } else {
            tracing::warn!("Attempted to set a status for a dependency that is not pending validation, this is a bug")
        }
    }

    /// Set the source of the dependency.
    ///
    /// This is used to track where the dependency was found.
    fn set_source(&mut self, new_source: CascadeSource) {
        if let Some(ValidationDependency { fetched_from, .. }) = &mut self.dependency {
            *fetched_from = new_source;
        }
    }
}

impl ValidationDependencyState {
    /// Get the action from the dependency state if it is present.
    pub(super) fn as_action(&self) -> Option<&Action> {
        self.dependency.as_ref().map(|d| match &d.value {
            ValidationDependencyValue::Action(a) => a.action(),
            ValidationDependencyValue::Warranted(WarrantedDep::Pending(action, _)) => {
                action.action()
            }
            ValidationDependencyValue::Warranted(WarrantedDep::Validated(action, _)) => {
                action.action()
            }
        })
    }

    /// Get the record from the dependency state if it is for a warranted record op that has been
    /// validated.
    pub(super) fn as_action_and_validation_status(&self) -> Option<(&Action, ValidationStatus)> {
        match &self.dependency {
            Some(d) => match &d.value {
                ValidationDependencyValue::Warranted(WarrantedDep::Validated(
                    a,
                    validation_status,
                )) => Some((a.action(), *validation_status)),
                _ => None,
            },
            None => None,
        }
    }
}

/// A validation dependency which is either an Action or a Record, and the source of the dependency.
#[derive(Clone, Debug)]
pub(super) struct ValidationDependency {
    value: ValidationDependencyValue,
    fetched_from: CascadeSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValidationDependencyType {
    Action,
    Warranted(ChainOpType),
}

#[derive(Clone, Debug)]
pub(super) enum ValidationDependencyValue {
    Action(SignedActionHashed),
    Warranted(WarrantedDep),
}

#[derive(Clone, Debug)]
pub(super) enum WarrantedDep {
    Pending(SignedActionHashed, ChainOpType),
    Validated(SignedActionHashed, ValidationStatus),
}
