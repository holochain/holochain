//! crate::dna::wasm is a module for managing webassembly code
//!  - within the in-memory dna struct
//!  - and serialized to json
use backtrace::Backtrace;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use tracing::*;

/// Represents web assembly code.
#[derive(Serialize, Deserialize, Clone, Eq)]
pub struct DnaWasm {
    /// the wasm bytes from a .wasm file
    pub code: bytes::Bytes,
}

/// A DnaWasm paired with its WasmHash
pub type DnaWasmHashed = HoloHashed<DnaWasm>;

impl HashableContent for DnaWasm {
    type HashType = hash_type::Wasm;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Wasm
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            self.try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl TryFrom<&DnaWasm> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(dna_wasm: &DnaWasm) -> Result<Self, Self::Error> {
        Ok(SerializedBytes::from(UnsafeBytes::from(
            dna_wasm.code.to_vec(),
        )))
    }
}
impl TryFrom<DnaWasm> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(dna_wasm: DnaWasm) -> Result<Self, Self::Error> {
        Self::try_from(&dna_wasm)
    }
}

impl TryFrom<SerializedBytes> for DnaWasm {
    type Error = SerializedBytesError;
    fn try_from(serialized_bytes: SerializedBytes) -> Result<Self, Self::Error> {
        Ok(DnaWasm {
            code: bytes::Bytes::from_owner(serialized_bytes.bytes().to_owned()),
        })
    }
}

impl DnaWasm {
    /// Provide basic placeholder for wasm entries in dna structs, used for testing only.
    pub fn new_invalid() -> Self {
        debug!(
            "DnaWasm::new_invalid() called from:\n{:?}",
            Backtrace::new()
        );
        DnaWasm {
            code: bytes::Bytes::new(),
        }
    }

    /// Accessor
    pub fn code(self) -> bytes::Bytes {
        self.code
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

impl From<bytes::Bytes> for DnaWasm {
    fn from(value: bytes::Bytes) -> Self {
        Self { code: value }
    }
}

impl From<Vec<u8>> for DnaWasm {
    fn from(value: Vec<u8>) -> Self {
        Self {
            code: bytes::Bytes::from_owner(value),
        }
    }
}
