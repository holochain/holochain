use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::zome::{FunctionName, ZomeName};
use serde::de::DeserializeOwned;
use std::collections::HashMap;

mod api;
use api::*;

mod ribosome;

use super::error::RibosomeResult;

#[derive(Debug)]
pub struct InlineDna<'f> {
    zomes: HashMap<ZomeName, InlineZome<'f>>,
}

impl<'f> InlineDna<'f> {
    pub fn new(zomes: HashMap<ZomeName, InlineZome<'f>>) -> Self {
        Self { zomes }
    }
}

#[derive(Default)]
pub struct InlineZome<'f> {
    functions: HashMap<FunctionName, InlineZomeFn<'f>>,
}

impl<'f> std::fmt::Debug for InlineZome<'f> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("<InlineZome>"))
    }
}

impl<'f> InlineZome<'f> {
    pub fn function<F, I, O>(mut self, name: &str, f: F) -> Self
    where
        F: Fn(InlineHostApi, I) -> RibosomeResult<O> + 'f,
        I: DeserializeOwned,
        O: Serialize,
    {
        let z =
            move |api: InlineHostApi, input: SerializedBytes| -> RibosomeResult<SerializedBytes> {
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

pub type InlineZomeFn<'f> =
    Box<dyn Fn(InlineHostApi, SerializedBytes) -> RibosomeResult<SerializedBytes> + 'f>;

#[cfg(test)]
mod tests {
    use super::*;
    use hdk3::prelude::*;
    use maplit::hashmap;

    #[test]
    fn can_create_inline_dna() {
        let zome = InlineZome::default().function("z1", |api, a: ()| {
            let hash: AnyDhtHash = todo!();
            api.get(hash, GetOptions::default())
        });
        let dna = InlineDna::new(hashmap! {
            "zome".into() => zome
        });
    }
}
