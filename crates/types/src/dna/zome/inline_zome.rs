//! A variant of Zome which is defined entirely by native, inline Rust code
//!
//! This type of Zome is only meant to be used for testing. It's designed to
//! make it easy to write a zome on-the-fly or programmatically, rather than
//! having to go through the heavy machinery of wasm compilation

use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::zome::FunctionName;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

use self::{api::InlineHostApi, error::InlineZomeResult};

pub mod api;
pub mod error;

/// An InlineZome, which consists
#[derive(Default)]
pub struct InlineZome {
    uuid: String,
    functions: HashMap<FunctionName, InlineZomeFn>,
}

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

impl InlineZome {
    /// Define a new inline zome function
    pub fn function<F, I, O>(mut self, name: &str, f: F) -> Self
    where
        F: Fn(InlineHostApi, I) -> InlineZomeResult<O> + 'static + Send + Sync,
        I: DeserializeOwned,
        O: Serialize,
    {
        let z = move |api: InlineHostApi,
                      input: SerializedBytes|
              -> InlineZomeResult<SerializedBytes> {
            let output = f(
                api,
                holochain_serialized_bytes::decode(input.bytes()).expect("TODO"),
            )?;
            Ok(SerializedBytes::from(UnsafeBytes::from(
                holochain_serialized_bytes::encode(&output).expect("TODO"),
            )))
        };
        self.functions.insert(name.into(), Box::new(z));
        self
    }
}

/// An inline zome function takes a Host API and an input, and produces an output.
pub type InlineZomeFn = Box<
    dyn Fn(InlineHostApi, SerializedBytes) -> InlineZomeResult<SerializedBytes>
        + 'static
        + Send
        + Sync,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::AnyDhtHash;
    use holochain_zome_types::prelude::GetOptions;
    use maplit::hashmap;

    #[test]
    fn can_create_inline_dna() {
        let zome = InlineZome::default().function("z1", |api, a: ()| {
            let hash: AnyDhtHash = todo!();
            api.get(hash, GetOptions::default())
        });
        // let dna = InlineDna::new(hashmap! {
        //     "zome".into() => zome
        // });
    }
}
