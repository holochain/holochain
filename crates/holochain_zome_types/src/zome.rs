//! A `Zome` is a module of app-defined code which can be run by Holochain.
//! A group of Zomes are composed to form a `DnaDef`.
//!
//! Real-world Holochain Zomes are written in Wasm.
//! This module also provides for an "inline" zome definition, which is written
//! using Rust closures, and is useful for quickly defining zomes on-the-fly
//! for tests.

pub use holochain_integrity_types::zome::*;

use holochain_serialized_bytes::prelude::*;

pub mod error;
#[cfg(feature = "full-dna-def")]
pub mod inline_zome;

use error::ZomeResult;

#[cfg(feature = "full-dna-def")]
use crate::InlineIntegrityZome;
#[cfg(feature = "full-dna-def")]
use error::ZomeError;
#[cfg(feature = "full-dna-def")]
use std::sync::Arc;

/// A Holochain Zome. Includes the ZomeDef as well as the name of the Zome.
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "full-dna-def", derive(shrinkwraprs::Shrinkwrap))]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
pub struct Zome<T = ZomeDef> {
    name: ZomeName,
    #[cfg_attr(feature = "full-dna-def", shrinkwrap(main_field))]
    def: T,
}

pub type IntegrityZome = Zome<IntegrityZomeDef>;

pub type CoordinatorZome = Zome<CoordinatorZomeDef>;

impl<T> Zome<T> {
    /// Constructor
    pub fn new(name: ZomeName, def: T) -> Self {
        Self { name, def }
    }

    /// Accessor
    pub fn zome_name(&self) -> &ZomeName {
        &self.name
    }

    /// Accessor
    pub fn zome_def(&self) -> &T {
        &self.def
    }

    /// Split into components
    pub fn into_inner(self) -> (ZomeName, T) {
        (self.name, self.def)
    }
}

impl IntegrityZome {
    /// Erase the type of [`Zome`] because you no longer
    /// need to know if this is an integrity or coordinator def.
    pub fn erase_type(self) -> Zome {
        Zome {
            name: self.name,
            def: self.def.erase_type(),
        }
    }
}

impl CoordinatorZome {
    /// Erase the type of [`Zome`] because you no longer
    /// need to know if this is an integrity or coordinator def.
    pub fn erase_type(self) -> Zome {
        Zome {
            name: self.name,
            def: self.def.erase_type(),
        }
    }
}

impl From<(ZomeName, ZomeDef)> for Zome {
    fn from(pair: (ZomeName, ZomeDef)) -> Self {
        Self::new(pair.0, pair.1)
    }
}

impl From<(ZomeName, IntegrityZomeDef)> for IntegrityZome {
    fn from(pair: (ZomeName, IntegrityZomeDef)) -> Self {
        Self::new(pair.0, pair.1)
    }
}

impl From<(ZomeName, CoordinatorZomeDef)> for CoordinatorZome {
    fn from(pair: (ZomeName, CoordinatorZomeDef)) -> Self {
        Self::new(pair.0, pair.1)
    }
}

impl<T> From<Zome<T>> for (ZomeName, T) {
    fn from(zome: Zome<T>) -> Self {
        zome.into_inner()
    }
}

impl<T> From<Zome<T>> for ZomeName {
    fn from(zome: Zome<T>) -> Self {
        zome.name
    }
}

impl From<IntegrityZome> for IntegrityZomeDef {
    fn from(zome: IntegrityZome) -> Self {
        zome.def
    }
}

impl From<CoordinatorZome> for CoordinatorZomeDef {
    fn from(zome: CoordinatorZome) -> Self {
        zome.def
    }
}

/// Just the definition of a Zome, without the name included. This exists
/// mainly for use in HashMaps where ZomeDefs are keyed by ZomeName.
///
/// NB: Only Wasm Zomes are valid to pass through round-trip serialization,
/// because Rust functions are not serializable. Hence, this enum serializes
/// as if it were a bare WasmZome, and when deserializing, only Wasm zomes
/// can be produced. InlineZomes are serialized as their UID, so that a
/// hash can be computed, but it is invalid to attempt to deserialize them
/// again.
///
/// In particular, a real-world DnaFile should only ever contain Wasm zomes!
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
// This can be untagged, since the only valid serialization target is WasmZome
#[serde(untagged, into = "ZomeDefSerialized")]
pub enum ZomeDef {
    /// A zome defined by Wasm bytecode
    Wasm(WasmZome),

    /// A zome defined by Rust closures. Cannot be deserialized.
    #[serde(skip_deserializing)]
    #[cfg(feature = "full-dna-def")]
    Inline(self::inline_zome::DynInlineZome),
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IntegrityZomeDef(ZomeDef);

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoordinatorZomeDef(ZomeDef);

/// The serialized form of a ZomeDef, which is identical for Wasm zomes, but
/// unwraps InlineZomes to just a bare UID.
#[derive(Serialize)]
#[serde(untagged)]
enum ZomeDefSerialized {
    Wasm(WasmZome),

    #[cfg(feature = "full-dna-def")]
    InlineUid(String),
}

impl From<ZomeDef> for ZomeDefSerialized {
    fn from(d: ZomeDef) -> Self {
        match d {
            ZomeDef::Wasm(zome) => Self::Wasm(zome),

            #[cfg(feature = "full-dna-def")]
            ZomeDef::Inline(zome) => Self::InlineUid(zome.0.uuid().clone()),
        }
    }
}

impl IntegrityZomeDef {
    pub fn as_any_zome_def(&self) -> &ZomeDef {
        &self.0
    }
}

impl CoordinatorZomeDef {
    pub fn as_any_zome_def(&self) -> &ZomeDef {
        &self.0
    }
}

#[cfg(feature = "full-dna-def")]
impl From<InlineIntegrityZome> for ZomeDef {
    fn from(iz: InlineIntegrityZome) -> Self {
        Self::Inline(inline_zome::DynInlineZome(Arc::new(iz)))
    }
}

#[cfg(feature = "full-dna-def")]
impl From<InlineIntegrityZome> for IntegrityZomeDef {
    fn from(iz: InlineIntegrityZome) -> Self {
        Self(ZomeDef::Inline(inline_zome::DynInlineZome(Arc::new(iz))))
    }
}

#[cfg(feature = "full-dna-def")]
impl From<crate::InlineCoordinatorZome> for ZomeDef {
    fn from(iz: crate::InlineCoordinatorZome) -> Self {
        Self::Inline(inline_zome::DynInlineZome(Arc::new(iz)))
    }
}

#[cfg(feature = "full-dna-def")]
impl From<crate::InlineCoordinatorZome> for CoordinatorZomeDef {
    fn from(iz: crate::InlineCoordinatorZome) -> Self {
        Self(ZomeDef::Inline(inline_zome::DynInlineZome(Arc::new(iz))))
    }
}

impl ZomeDef {
    /// If this is a Wasm zome, return the WasmHash.
    /// If not, return an error with the provided zome name
    //
    // NB: argument uses underscore here because without full-dna-def feature,
    //     the arg is unused.
    pub fn wasm_hash(&self, _zome_name: &ZomeName) -> ZomeResult<holo_hash::WasmHash> {
        match self {
            ZomeDef::Wasm(WasmZome { wasm_hash }) => Ok(wasm_hash.clone()),
            #[cfg(feature = "full-dna-def")]
            ZomeDef::Inline(_) => Err(ZomeError::NonWasmZome(_zome_name.clone())),
        }
    }
}

impl IntegrityZomeDef {
    pub fn wasm_hash(&self, zome_name: &ZomeName) -> ZomeResult<holo_hash::WasmHash> {
        self.0.wasm_hash(zome_name)
    }
}

impl CoordinatorZomeDef {
    pub fn wasm_hash(&self, zome_name: &ZomeName) -> ZomeResult<holo_hash::WasmHash> {
        self.0.wasm_hash(zome_name)
    }
}

impl From<ZomeDef> for IntegrityZomeDef {
    fn from(z: ZomeDef) -> Self {
        Self(z)
    }
}

impl From<ZomeDef> for CoordinatorZomeDef {
    fn from(z: ZomeDef) -> Self {
        Self(z)
    }
}

#[cfg(feature = "test_utils")]
impl<'a> arbitrary::Arbitrary<'a> for ZomeDef {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::Wasm(WasmZome::arbitrary(u)?))
    }
}

/// A zome defined by Wasm bytecode
#[derive(
    Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, SerializedBytes,
)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
pub struct WasmZome {
    /// The WasmHash representing the WASM byte code for this zome.
    pub wasm_hash: holo_hash::WasmHash,
}

impl WasmZome {
    /// Constructor
    pub fn new(wasm_hash: holo_hash::WasmHash) -> Self {
        Self { wasm_hash }
    }
}

impl ZomeDef {
    /// create a Zome from a holo_hash WasmHash instead of a holo_hash one
    pub fn from_hash(wasm_hash: holo_hash::WasmHash) -> Self {
        Self::Wasm(WasmZome { wasm_hash })
    }
}

impl IntegrityZomeDef {
    pub fn from_hash(wasm_hash: holo_hash::WasmHash) -> Self {
        Self(ZomeDef::from_hash(wasm_hash))
    }

    /// Erase the type of [`ZomeDef`] because you no longer
    /// need to know if this is an integrity or coordinator def.
    pub fn erase_type(self) -> ZomeDef {
        self.0
    }
}

impl CoordinatorZomeDef {
    pub fn from_hash(wasm_hash: holo_hash::WasmHash) -> Self {
        Self(ZomeDef::from_hash(wasm_hash))
    }

    /// Erase the type of [`ZomeDef`] because you no longer
    /// need to know if this is an integrity or coordinator def.
    pub fn erase_type(self) -> ZomeDef {
        self.0
    }
}
