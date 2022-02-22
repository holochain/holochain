//! A variant of Zome which is defined entirely by native, inline Rust code
//!
//! This type of Zome is only meant to be used for testing. It's designed to
//! make it easy to write a zome on-the-fly or programmatically, rather than
//! having to go through the heavy machinery of wasm compilation

use self::error::InlineZomeResult;
use crate::prelude::*;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub mod error;

pub type BoxApi = Box<dyn HostFnApiT>;

/// An InlineZome, which consists
pub struct InlineZome {
    /// Since closures cannot be serialized, we include a UID which
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
    pub(super) callbacks: HashMap<FunctionName, InlineZomeFn>,
}

impl InlineZome {
    /// Create a new zome with the given UID
    pub fn new<S: Into<String>>(uuid: S, entry_defs: Vec<EntryDef>) -> Self {
        let entry_defs_callback =
            move |_, _: ()| Ok(EntryDefsCallbackResult::Defs(entry_defs.clone().into()));
        Self {
            uuid: uuid.into(),
            callbacks: HashMap::new(),
        }
        .callback("entry_defs", Box::new(entry_defs_callback))
    }
    /// Create a new zome with a unique random UID
    pub fn new_unique(entry_defs: Vec<EntryDef>) -> Self {
        Self::new(nanoid::nanoid!(), entry_defs)
    }

    pub fn callbacks(&self) -> Vec<FunctionName> {
        let mut keys: Vec<FunctionName> = self.callbacks.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Define a new zome function or callback with the given name
    pub fn callback<F, I, O>(mut self, name: &str, f: F) -> Self
    where
        F: Fn(BoxApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned + std::fmt::Debug,
        O: Serialize + std::fmt::Debug,
    {
        let z = move |api: BoxApi, input: ExternIO| -> InlineZomeResult<ExternIO> {
            Ok(ExternIO::encode(f(api, input.decode()?)?)?)
        };
        if self.callbacks.insert(name.into(), Box::new(z)).is_some() {
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
        if let Some(f) = self.callbacks.get(name) {
            Ok(Some(f(api, input)?))
        } else {
            Ok(None)
        }
    }

    /// Accessor
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }
}

/// An inline zome function takes a Host API and an input, and produces an output.
pub type InlineZomeFn =
    Box<dyn Fn(BoxApi, ExternIO) -> InlineZomeResult<ExternIO> + 'static + Send + Sync>;

impl std::fmt::Debug for InlineZome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("<InlineZome {}>", self.uuid))
    }
}

impl PartialEq for InlineZome {
    fn eq(&self, other: &InlineZome) -> bool {
        self.uuid == other.uuid
    }
}

impl PartialOrd for InlineZome {
    fn partial_cmp(&self, other: &InlineZome) -> Option<std::cmp::Ordering> {
        Some(self.uuid.cmp(&other.uuid))
    }
}

impl Eq for InlineZome {}

impl Ord for InlineZome {
    fn cmp(&self, other: &InlineZome) -> std::cmp::Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl std::hash::Hash for InlineZome {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
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
        let zome = InlineZome::new("", vec![]).callback("zome_fn_1", |api, a: ()| {
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
