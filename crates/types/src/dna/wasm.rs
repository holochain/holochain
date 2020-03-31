//! sx_types::dna::wasm is a module for managing webassembly code
//!  - within the in-memory dna struct
//!  - and serialized to json
use crate::error::SkunkError;
use backtrace::Backtrace;
use base64;
use derive_more::{AsRef, Deref, From, Into};
use holochain_serialized_bytes::prelude::*;
use serde::{
    self,
    de::{Deserializer, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::{Arc, RwLock},
};
use tracing::*;

/// use wasmi::Module;
/// TODO: dummy shim for wasm, will be replaced by wasmer soon
pub struct Module;

/// Wrapper around wasmi::Module since it does not implement Clone, Debug, PartialEq, Eq,
/// which are all needed to add it to the DnaWasm below, and hence to the state.
#[derive(Clone)]
pub struct ModuleArc(Arc<Module>);
impl ModuleArc {
    /// Construct a new ModuleArc newtype - just an Arc around Module
    pub fn new(module: Module) -> Self {
        ModuleArc(Arc::new(module))
    }
}

impl PartialEq for ModuleArc {
    fn eq(&self, _other: &ModuleArc) -> bool {
        //*self == *other
        false
    }
}

impl Eq for ModuleArc {}

impl Deref for ModuleArc {
    type Target = Arc<Module>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for ModuleArc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ModuleMutex")
    }
}

/// Private helper for converting binary WebAssembly into base64 serialized string.
fn _vec_u8_to_b64_str<S>(data: &Arc<Vec<u8>>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let b64 = base64::encode(data.as_ref());
    s.serialize_str(&b64)
}

/// Private helper for converting base64 string into binary WebAssembly.
fn _b64_str_to_vec_u8<'de, D>(d: D) -> Result<Arc<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    /// visitor struct needed for serde deserialization
    struct Z;

    impl<'de> Visitor<'de> for Z {
        type Value = Vec<u8>;

        /// we only want to accept strings
        fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            formatter.write_str("string")
        }

        /// if we get a string, try to base64 decode into binary
        fn visit_str<E>(self, value: &str) -> Result<Vec<u8>, E>
        where
            E: serde::de::Error,
        {
            match base64::decode(value) {
                Ok(v) => Ok(v),
                Err(e) => Err(serde::de::Error::custom(e)),
            }
        }
    }

    Ok(Arc::new(d.deserialize_any(Z)?))
}

/// Represents web assembly code.
#[derive(Serialize, Deserialize, Clone, PartialEq, Hash, AsRef, From, Into, Deref)]
pub struct DnaWasm(Vec<u8>);

impl From<UnsafeBytes> for DnaWasm {
    fn from(ub: UnsafeBytes) -> DnaWasm {
        DnaWasm(ub.into())
    }
}

impl From<DnaWasm> for UnsafeBytes {
    fn from(wasm: DnaWasm) -> UnsafeBytes {
        UnsafeBytes::from(wasm)
    }
}

impl AsRef<[u8]> for DnaWasm {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl DnaWasm {
    /// Provide basic placeholder for wasm entries in dna structs, used for testing only.
    // TODO, remove?
    pub fn new_invalid() -> Self {
        debug!(
            "DnaWasm::new_invalid() called from:\n{:?}",
            Backtrace::new()
        );
        DnaWasm(vec![])
    }
}

impl fmt::Debug for DnaWasm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<<<DNA WASM CODE>>>")
    }
}

impl DnaWasm {
    /// Creates a new instance from given WASM binary
    pub fn from_bytes(wasm: Vec<u8>) -> Self {
        DnaWasm(wasm)
    }
}
