//! A variant of Zome which is defined entirely by native, inline Rust code
//!
//! This type of Zome is only meant to be used for testing. It's designed to
//! make it easy to write a zome on-the-fly or programmatically, rather than
//! having to go through the heavy machinery of wasm compilation

use self::error::InlineZomeResult;
use crate::prelude::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

pub mod error;

pub type BoxApi = Box<dyn HostFnApiT>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A type marker for an integrity [`InlineZome`].
pub struct IntegrityZomeMarker;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A type marker for a coordinator [`InlineZome`].
pub struct CoordinatorZomeMarker;

pub type InlineIntegrityZome = InlineZome<IntegrityZomeMarker>;
pub type InlineCoordinatorZome = InlineZome<CoordinatorZomeMarker>;

/// An InlineZome, which consists
pub struct InlineZome<T> {
    /// Inline zome type marker.
    _t: PhantomData<T>,
    /// Since closures cannot be serialized, we include a network seed which
    /// is the only part of an InlineZome that gets serialized.
    /// This uuid becomes part of the determination of the DnaHash
    /// that it is a part of.
    /// Think of it as a stand-in for the WasmHash of a WasmZome.
    pub(super) uuid: String,

    // /// The EntryDefs returned by the `entry_defs` callback function,
    // /// which will be automatically provided
    // pub(super) entry_defs: EntryDefs,
    /// The collection of closures which define this zome.
    /// These callbacks are directly called by the Ribosome.
    pub(super) functions: HashMap<FunctionName, InlineZomeFn>,

    /// Global values for this zome.
    pub(super) globals: HashMap<String, u8>,
}

impl<T> InlineZome<T> {
    /// Inner constructor.
    fn new_inner<S: Into<String>>(uuid: S) -> Self {
        Self {
            _t: PhantomData,
            uuid: uuid.into(),
            functions: HashMap::new(),
            globals: HashMap::new(),
        }
    }

    pub fn functions(&self) -> Vec<FunctionName> {
        let mut keys: Vec<FunctionName> = self.functions.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Define a new zome function or callback with the given name
    pub fn function<F, I, O>(mut self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        let z = move |api: BoxApi, input: ExternIO| -> InlineZomeResult<ExternIO> {
            Ok(ExternIO::encode(f(api, input.decode()?)?)?)
        };
        if self.functions.insert(name.into(), Box::new(z)).is_some() {
            tracing::warn!("Replacing existing InlineZome callback '{}'", name);
        };
        self
    }

    /// Make a call to an inline zome callback.
    /// If the callback doesn't exist, return None.
    pub fn maybe_call(
        &self,
        api: BoxApi,
        name: &FunctionName,
        input: ExternIO,
    ) -> InlineZomeResult<Option<ExternIO>> {
        if let Some(f) = self.functions.get(name) {
            Ok(Some(f(api, input)?))
        } else {
            Ok(None)
        }
    }

    /// Accessor
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }

    /// Set a global value for this zome.
    pub fn set_global(mut self, name: impl Into<String>, val: u8) -> Self {
        self.globals.insert(name.into(), val);
        self
    }
}

impl InlineIntegrityZome {
    /// Create a new integrity zome with the given network seed
    pub fn new<S: Into<String>>(uuid: S, entry_defs: Vec<EntryDef>, num_link_types: u8) -> Self {
        let num_entry_types = entry_defs.len();
        let entry_defs_callback =
            move |_, _: ()| Ok(EntryDefsCallbackResult::Defs(entry_defs.clone().into()));
        Self::new_inner(uuid)
            .function("entry_defs", Box::new(entry_defs_callback))
            .set_global("__num_entry_types", num_entry_types.try_into().unwrap())
            .set_global("__num_link_types", num_link_types)
    }
    /// Create a new integrity zome with a unique random network seed
    pub fn new_unique(entry_defs: Vec<EntryDef>, num_link_types: u8) -> Self {
        Self::new(nanoid::nanoid!(), entry_defs, num_link_types)
    }
}

impl InlineCoordinatorZome {
    /// Create a new coordinator zome with the given network seed
    pub fn new<S: Into<String>>(uuid: S) -> Self {
        Self::new_inner(uuid)
    }
    /// Create a new coordinator zome with a unique random network seed
    pub fn new_unique() -> Self {
        Self::new(nanoid::nanoid!())
    }
}

#[derive(Debug, Clone)]
/// An inline zome clonable type object.
pub struct DynInlineZome(pub Arc<dyn InlineZomeT + Send + Sync>);

pub trait InlineZomeT: std::fmt::Debug {
    /// Get the functions for this [`InlineZome`].
    fn functions(&self) -> Vec<FunctionName>;

    /// Make a call to an inline zome callback.
    /// If the callback doesn't exist, return None.
    fn maybe_call(
        &self,
        api: BoxApi,
        name: &FunctionName,
        input: ExternIO,
    ) -> InlineZomeResult<Option<ExternIO>>;

    /// Accessor
    fn uuid(&self) -> String;

    /// Get a global value for this zome.
    fn get_global(&self, name: &str) -> Option<u8>;
}

/// An inline zome function takes a Host API and an input, and produces an output.
pub type InlineZomeFn =
    Box<dyn Fn(BoxApi, ExternIO) -> InlineZomeResult<ExternIO> + 'static + Send + Sync>;

impl<T: std::fmt::Debug> InlineZomeT for InlineZome<T> {
    fn functions(&self) -> Vec<FunctionName> {
        self.functions()
    }

    fn maybe_call(
        &self,
        api: BoxApi,
        name: &FunctionName,
        input: ExternIO,
    ) -> InlineZomeResult<Option<ExternIO>> {
        self.maybe_call(api, name, input)
    }

    fn uuid(&self) -> String {
        self.uuid()
    }

    fn get_global(&self, name: &str) -> Option<u8> {
        self.globals.get(name).copied()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for InlineZome<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("<InlineZome {}>", self.uuid))
    }
}

impl<T: PartialEq> PartialEq for InlineZome<T> {
    fn eq(&self, other: &InlineZome<T>) -> bool {
        self.uuid == other.uuid
    }
}

impl PartialEq for DynInlineZome {
    fn eq(&self, other: &DynInlineZome) -> bool {
        self.0.uuid() == other.0.uuid()
    }
}

impl<T: PartialOrd> PartialOrd for InlineZome<T> {
    fn partial_cmp(&self, other: &InlineZome<T>) -> Option<std::cmp::Ordering> {
        Some(self.uuid.cmp(&other.uuid))
    }
}

impl PartialOrd for DynInlineZome {
    fn partial_cmp(&self, other: &DynInlineZome) -> Option<std::cmp::Ordering> {
        Some(self.0.uuid().cmp(&other.0.uuid()))
    }
}

impl<T: Eq> Eq for InlineZome<T> {}

impl Eq for DynInlineZome {}

impl<T: Ord> Ord for InlineZome<T> {
    fn cmp(&self, other: &InlineZome<T>) -> std::cmp::Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl Ord for DynInlineZome {
    fn cmp(&self, other: &DynInlineZome) -> std::cmp::Ordering {
        self.0.uuid().cmp(&other.0.uuid())
    }
}

impl<T: std::hash::Hash> std::hash::Hash for InlineZome<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl std::hash::Hash for DynInlineZome {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.uuid().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::GetOptions;
    use holo_hash::AnyDhtHash;

    #[test]
    #[allow(unused_variables, unreachable_code)]
    fn can_create_inline_dna() {
        let zome = InlineIntegrityZome::new("", vec![], 0).function("zome_fn_1", |api, a: ()| {
            let hash: AnyDhtHash = todo!();
            Ok(api
                .get(vec![GetInput::new(hash, GetOptions::default())])
                .expect("TODO after crate re-org"))
        });
        // let dna = InlineDna::new(hashmap! {
        //     "zome".into() => zome
        // });
    }
}
