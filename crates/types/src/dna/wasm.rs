//! sx_types::dna::wasm is a module for managing webassembly code
//!  - within the in-memory dna struct
//!  - and serialized to json
use backtrace::Backtrace;

use base64;
use serde::{
    self,
    de::{Deserializer, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};
use tracing::*;

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
#[derive(Serialize, Deserialize, Clone)]
pub struct DnaWasm {
    /// The actual binary WebAssembly bytecode goes here.
    #[serde(
        serialize_with = "_vec_u8_to_b64_str",
        deserialize_with = "_b64_str_to_vec_u8"
    )]
    pub code: Arc<Vec<u8>>,
}

impl DnaWasm {
    /// Provide basic placeholder for wasm entries in dna structs, used for testing only.
    pub fn new_invalid() -> Self {
        debug!(
            "DnaWasm::new_invalid() called from:\n{:?}",
            Backtrace::new()
        );
        DnaWasm {
            code: Arc::new(vec![]),
        }
    }
}

impl fmt::Debug for DnaWasm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<<<DNA WASM CODE>>>")
    }
}

impl PartialEq for DnaWasm {
    fn eq(&self, other: &DnaWasm) -> bool {
        self.code == other.code
    }
}

impl Hash for DnaWasm {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.code.hash(state);
    }
}

impl DnaWasm {
    /// Creates a new instance from given WASM binary
    pub fn from_bytes(wasm: Vec<u8>) -> Self {
        DnaWasm {
            code: Arc::new(wasm),
        }
    }
}
