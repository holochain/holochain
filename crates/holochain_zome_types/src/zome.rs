//! A `Zome` is a module of app-defined code which can be run by Holochain.
//! A group of Zomes are composed to form a `DnaDef`.
//!
//! Real-world Holochain Zomes are written in Wasm.
//! This module also provides for an "inline" zome definition, which is written
//! using Rust closures, and is useful for quickly defining zomes on-the-fly
//! for tests.

pub use holochain_integrity_types::zome::*;
use holochain_serialized_bytes::prelude::*;

mod error;
pub use error::*;

#[cfg(feature = "full-dna-def")]
pub mod inline_zome;

#[cfg(feature = "full-dna-def")]
use inline_zome::InlineIntegrityZome;

/// A Holochain Zome. Includes the ZomeDef as well as the name of the Zome.
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "full-dna-def", derive(shrinkwraprs::Shrinkwrap))]
pub struct Zome<T: Send + Sync = ZomeDef> {
    pub name: ZomeName,
    #[cfg_attr(feature = "full-dna-def", shrinkwrap(main_field))]
    pub def: T,
}

pub type IntegrityZome = Zome<IntegrityZomeDef>;

pub type CoordinatorZome = Zome<CoordinatorZomeDef>;

// Use an integrity zome as a coordinator zome,
// for cases where integrity zomes define zome functions
impl From<IntegrityZome> for CoordinatorZome {
    fn from(zome: IntegrityZome) -> Self {
        Zome {
            name: zome.name,
            def: CoordinatorZomeDef(zome.def.0),
        }
    }
}

impl<T: Send + Sync> Zome<T> {
    /// Constructor
    pub fn new(name: ZomeName, def: T) -> Self {
        Self { name, def }
    }

    /// Accessor
    pub fn zome_name(&self) -> &ZomeName {
        &self.name
    }

    pub fn zome_name_mut(&mut self) -> &mut ZomeName {
        &mut self.name
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

    /// Add a dependency to this zome.
    pub fn set_dependency(&mut self, zome_name: impl Into<ZomeName>) {
        self.def.set_dependency(zome_name);
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

impl<T: Send + Sync> From<Zome<T>> for (ZomeName, T) {
    fn from(zome: Zome<T>) -> Self {
        zome.into_inner()
    }
}

impl<T: Send + Sync> From<Zome<T>> for ZomeName {
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

/// A zome defined by Wasm bytecode
// TODO: move to `holochain_types`
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WasmZomeDef {
    /// The WasmHash representing the WASM byte code for this zome.
    pub wasm_hash: holo_hash::WasmHash,

    /// The zome dependencies
    pub dependencies: Vec<ZomeName>,
}

/// A zome defined by inline Rust code
#[cfg(feature = "full-dna-def")]
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InlineZomeDef {
    pub inline_hash: holo_hash::InlineHash,

    pub dependencies: Vec<ZomeName>,
}

/// Just the definition of a Zome, without the name included. This exists
/// mainly for use in HashMaps where ZomeDefs are keyed by ZomeName.
///
/// NB: A `ZomeDef` only describes a zome, it does not hold the executable code.
/// A WASM zome carries its `WasmHash` and an inline zome carries an `InlineHash`
/// stand-in (see `InlineZomeDef`). Both variants round-trip through
/// serialization, but deserializing an inline zome only recovers its identifying
/// hash, not the original Rust closures, so the inline implementation must be
/// supplied separately (the closures live on `DnaFile`).
///
/// In particular, a real-world DnaFile should only ever contain Wasm zomes!
// TODO: move to `holochain_types`
#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZomeDef {
    /// A zome defined by Wasm bytecode
    Wasm(WasmZomeDef),

    /// A zome defined by Rust closures, identified here only by its `InlineHash`.
    #[cfg(feature = "full-dna-def")]
    Inline(InlineZomeDef),
}

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IntegrityZomeDef(ZomeDef);

#[derive(Serialize, Deserialize, Hash, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoordinatorZomeDef(ZomeDef);

impl IntegrityZomeDef {
    pub fn as_any_zome_def(&self) -> &ZomeDef {
        &self.0
    }
}

impl CoordinatorZomeDef {
    /// Use this as any [`ZomeDef`].
    pub fn as_any_zome_def(&self) -> &ZomeDef {
        &self.0
    }

    /// Add a dependency to this zome.
    pub fn set_dependency(&mut self, zome_name: impl Into<ZomeName>) {
        match &mut self.0 {
            ZomeDef::Wasm(WasmZomeDef { dependencies, .. }) => dependencies.push(zome_name.into()),

            #[cfg(feature = "full-dna-def")]
            ZomeDef::Inline(InlineZomeDef { dependencies, .. }) => {
                dependencies.push(zome_name.into())
            }
        }
    }
}

#[cfg(feature = "full-dna-def")]
impl From<InlineIntegrityZome> for ZomeDef {
    fn from(iz: InlineIntegrityZome) -> Self {
        Self::Inline(InlineZomeDef {
            inline_hash: iz.hash,
            dependencies: Vec::with_capacity(0),
        })
    }
}

#[cfg(feature = "full-dna-def")]
impl From<InlineIntegrityZome> for IntegrityZomeDef {
    fn from(iz: InlineIntegrityZome) -> Self {
        Self(iz.into())
    }
}

// TODO poorly defined conversion, cannot capture dependencies
#[cfg(feature = "full-dna-def")]
impl From<crate::prelude::InlineCoordinatorZome> for ZomeDef {
    fn from(iz: crate::prelude::InlineCoordinatorZome) -> Self {
        Self::Inline(InlineZomeDef {
            inline_hash: iz.hash,
            dependencies: Default::default(),
        })
    }
}

#[cfg(feature = "full-dna-def")]
impl From<crate::prelude::InlineCoordinatorZome> for CoordinatorZomeDef {
    fn from(iz: crate::prelude::InlineCoordinatorZome) -> Self {
        Self(iz.into())
    }
}

impl ZomeDef {
    /// Get the [`ZomeHash`](holo_hash::ZomeHash) for this zome def.
    pub fn zome_hash(&self) -> ZomeResult<holo_hash::ZomeHash> {
        match self {
            ZomeDef::Wasm(WasmZomeDef { wasm_hash, .. }) => Ok(wasm_hash.clone().into()),

            #[cfg(feature = "full-dna-def")]
            ZomeDef::Inline(InlineZomeDef { inline_hash, .. }) => Ok(inline_hash.clone().into()),
        }
    }

    /// Get the dependencies of this zome.
    pub fn dependencies(&self) -> &[ZomeName] {
        match self {
            ZomeDef::Wasm(WasmZomeDef { dependencies, .. }) => &dependencies[..],

            #[cfg(feature = "full-dna-def")]
            ZomeDef::Inline(InlineZomeDef { dependencies, .. }) => &dependencies[..],
        }
    }
}

impl IntegrityZomeDef {
    pub fn zome_hash(&self) -> ZomeResult<holo_hash::ZomeHash> {
        self.0.zome_hash()
    }
}

impl CoordinatorZomeDef {
    pub fn zome_hash(&self) -> ZomeResult<holo_hash::ZomeHash> {
        self.0.zome_hash()
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

impl WasmZomeDef {
    /// Constructor
    pub fn new(wasm_hash: holo_hash::WasmHash, dependencies: Option<Vec<ZomeName>>) -> Self {
        Self {
            wasm_hash,
            dependencies: dependencies.unwrap_or_default(),
        }
    }
}

impl ZomeDef {
    /// create a Zome from a holo_hash WasmHash instead of a holo_hash one
    pub fn from_hash(wasm_hash: holo_hash::WasmHash) -> Self {
        Self::Wasm(WasmZomeDef {
            wasm_hash,
            dependencies: Default::default(),
        })
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
