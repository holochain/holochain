//! Information about the current zome and dna.
use crate::action::ZomeIndex;
use crate::zome::ZomeName;
use crate::EntryDefIndex;
use crate::EntryDefs;
use crate::FunctionName;
use crate::LinkType;
use crate::Timestamp;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;
use std::time::Duration;

#[cfg(test)]
mod test;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeIndex,
    pub properties: SerializedBytes,
    pub entry_defs: EntryDefs,
    // @todo make this include function signatures when they exist.
    pub extern_fns: Vec<FunctionName>,
    /// All the zome types that are in scope for this zome.
    pub zome_types: ScopedZomeTypesSet,
}

impl ZomeInfo {
    /// Create a new ZomeInfo.
    pub fn new(
        name: ZomeName,
        id: ZomeIndex,
        properties: SerializedBytes,
        entry_defs: EntryDefs,
        extern_fns: Vec<FunctionName>,
        zome_types: ScopedZomeTypesSet,
    ) -> Self {
        Self {
            name,
            id,
            properties,
            entry_defs,
            extern_fns,
            zome_types,
        }
    }
}

/// Placeholder for a real network seed type. See [`DnaDef`].
pub type NetworkSeed = String;

#[allow(dead_code)]
const fn standard_quantum_time() -> Duration {
    // TODO - put this in a common place that is imported
    //        from both this crate and kitsune_p2p_dht
    //        we do *not* want kitsune_p2p_dht imported into
    //        this crate, because that pulls getrandom into
    //        something that is supposed to be compiled
    //        into integrity wasms.
    Duration::from_secs(60 * 5)
}

/// Modifiers of this DNA - the network seed, properties and origin time - as
/// opposed to the actual DNA code. These modifiers are included in the DNA
/// hash computation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
#[cfg_attr(feature = "full-dna-def", derive(derive_builder::Builder))]
pub struct DnaModifiers {
    /// The network seed of a DNA is included in the computation of the DNA hash.
    /// The DNA hash in turn determines the network peers and the DHT, meaning
    /// that only peers with the same DNA hash of a shared DNA participate in the
    /// same network and co-create the DHT. To create a separate DHT for the DNA,
    /// a unique network seed can be specified.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    pub network_seed: NetworkSeed,

    /// Any arbitrary application properties can be included in this object.
    #[cfg_attr(feature = "full-dna-def", builder(default = "().try_into().unwrap()"))]
    pub properties: SerializedBytes,

    /// The time used to denote the origin of the network, used to calculate
    /// time windows during gossip.
    /// All Action timestamps must come after this time.
    #[cfg_attr(feature = "full-dna-def", builder(default = "Timestamp::now()"))]
    pub origin_time: Timestamp,

    /// The smallest unit of time used for gossip time windows.
    /// You probably don't need to change this.
    #[cfg_attr(feature = "full-dna-def", builder(default = "standard_quantum_time()"))]
    #[cfg_attr(feature = "full-dna-def", serde(default = "standard_quantum_time"))]
    pub quantum_time: Duration,
}

impl DnaModifiers {
    /// Replace fields in the modifiers with any Some fields in the argument.
    /// None fields remain unchanged.
    pub fn update(mut self, modifiers: DnaModifiersOpt) -> DnaModifiers {
        self.network_seed = modifiers.network_seed.unwrap_or(self.network_seed);
        self.properties = modifiers.properties.unwrap_or(self.properties);
        self.origin_time = modifiers.origin_time.unwrap_or(self.origin_time);
        self.quantum_time = modifiers.quantum_time.unwrap_or(self.quantum_time);
        self
    }
}

/// [`DnaModifiers`] options of which all are optional.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct DnaModifiersOpt<P = SerializedBytes> {
    /// see [`DnaModifiers`]
    pub network_seed: Option<NetworkSeed>,
    /// see [`DnaModifiers`]
    pub properties: Option<P>,
    /// see [`DnaModifiers`]
    pub origin_time: Option<Timestamp>,
    /// see [`DnaModifiers`]
    pub quantum_time: Option<Duration>,
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> Default
    for DnaModifiersOpt<P>
{
    fn default() -> Self {
        Self::none()
    }
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> DnaModifiersOpt<P> {
    /// Constructor with all fields set to `None`
    pub fn none() -> Self {
        Self {
            network_seed: None,
            properties: None,
            origin_time: None,
            quantum_time: None,
        }
    }

    /// Serialize the properties field into SerializedBytes
    pub fn serialized(self) -> Result<DnaModifiersOpt<SerializedBytes>, E> {
        let Self {
            network_seed,
            properties,
            origin_time,
            quantum_time,
        } = self;
        let properties = if let Some(p) = properties {
            Some(p.try_into()?)
        } else {
            None
        };
        Ok(DnaModifiersOpt {
            network_seed,
            properties,
            origin_time,
            quantum_time,
        })
    }

    /// Return a modified form with the `network_seed` field set
    pub fn with_network_seed(mut self, network_seed: NetworkSeed) -> Self {
        self.network_seed = Some(network_seed);
        self
    }

    /// Return a modified form with the `properties` field set
    pub fn with_properties(mut self, properties: P) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Return a modified form with the `origin_time` field set
    pub fn with_origin_time(mut self, origin_time: Timestamp) -> Self {
        self.origin_time = Some(origin_time);
        self
    }

    /// Return a modified form with the `quantum_time` field set
    pub fn with_quantum_time(mut self, quantum_time: Duration) -> Self {
        self.quantum_time = Some(quantum_time);
        self
    }

    /// Check if at least one of the options is set.
    pub fn has_some_option_set(&self) -> bool {
        self.network_seed.is_some() || self.properties.is_some() || self.origin_time.is_some()
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Information about the current DNA.
pub struct DnaInfoV1 {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The properties of this DNA.
    pub properties: SerializedBytes,
    // In ZomeIndex order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Information about the current DNA.
pub struct DnaInfoV2 {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The modifiers for this DNA.
    pub modifiers: DnaModifiers,
    // In ZomeIndex order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}

/// Convenience alias to the latest `DnaInfoN`.
pub type DnaInfo = DnaInfoV2;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
/// The set of [`EntryDefIndex`] and [`LinkType`]s in scope for the calling zome.
pub struct ScopedZomeTypesSet {
    /// All the entry [`EntryDefIndex`]s in scope for this zome.
    pub entries: ScopedZomeTypes<EntryDefIndex>,
    /// All the entry [`LinkType`]s in scope for this zome.
    pub links: ScopedZomeTypes<LinkType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// zome types that are in scope for the calling zome.
pub struct ScopedZomeTypes<T>(pub Vec<(ZomeIndex, Vec<T>)>);

impl<T> Default for ScopedZomeTypes<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A key to the [`ScopedZomeTypes`] container.
pub struct ZomeTypesKey<T>
where
    T: U8Index + Copy,
{
    /// The index into the [`ZomeIndex`] vec.
    pub zome_index: ZomeDependencyIndex,
    /// The index into the types vec.
    pub type_index: T,
}

/// A key to the [`ScopedZomeTypes<EntryDefIndex>`] container.
pub type ZomeEntryTypesKey = ZomeTypesKey<EntryDefIndex>;
/// A key to the [`ScopedZomeTypes<LinkType>`] container.
pub type ZomeLinkTypesKey = ZomeTypesKey<LinkType>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// The index into the [`ZomeIndex`] vec.
pub struct ZomeDependencyIndex(pub u8);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A type with the zome that it is defined in.
pub struct ScopedZomeType<T> {
    /// The zome that defines this type.
    pub zome_index: ZomeIndex,
    /// The type that is defined.
    pub zome_type: T,
}

/// An [`EntryDefIndex`] within the scope of the zome where it's defined.
pub type ScopedEntryDefIndex = ScopedZomeType<EntryDefIndex>;
/// A [`LinkType`] within the scope of the zome where it's defined.
pub type ScopedLinkType = ScopedZomeType<LinkType>;

impl<T> ScopedZomeTypes<T>
where
    T: U8Index + Copy,
{
    /// Get a [`ScopedZomeType`] if one exist at this key.
    pub fn get<K>(&self, key: K) -> Option<ScopedZomeType<T>>
    where
        K: Into<ZomeTypesKey<T>>,
    {
        let key = key.into();
        self.0
            .get(key.zome_index.index())
            .and_then(|(zome_index, types)| {
                types
                    .get(key.type_index.index())
                    .copied()
                    .map(|zome_type| ScopedZomeType {
                        zome_index: *zome_index,
                        zome_type,
                    })
            })
    }

    /// Find the user type in the given iterator that matches this [`ScopedZomeType`].
    pub fn find<I, K>(&self, iter: I, scoped_type: ScopedZomeType<T>) -> Option<I::Item>
    where
        I: IntoIterator<Item = K>,
        K: Into<ZomeTypesKey<T>> + Copy,
        T: PartialEq,
    {
        iter.into_iter()
            .find_map(|key| (self.get(key)? == scoped_type).then_some(key))
    }

    /// Find the [`ZomeTypesKey`] for this [`ScopedZomeType`].
    pub fn find_key(&self, scoped_type: ScopedZomeType<T>) -> Option<ZomeTypesKey<T>>
    where
        T: PartialEq,
        T: From<u8>,
    {
        self.0
            .iter()
            .position(|(zome_index, _)| *zome_index == scoped_type.zome_index)
            .and_then(|zome_index| {
                // Safe to index because we just checked position.
                self.0[zome_index]
                    .1
                    .iter()
                    .position(|zome_type| *zome_type == scoped_type.zome_type)
                    .and_then(|type_index| {
                        Some(ZomeTypesKey {
                            zome_index: u8::try_from(zome_index).ok()?.into(),
                            type_index: u8::try_from(type_index).ok()?.into(),
                        })
                    })
            })
    }

    /// Get all the [`ZomeIndex`] dependencies for the calling zome.
    pub fn dependencies(&self) -> impl Iterator<Item = ZomeIndex> + '_ {
        self.0.iter().map(|(zome_index, _)| *zome_index)
    }
}

impl From<EntryDefIndex> for ZomeEntryTypesKey {
    fn from(type_index: EntryDefIndex) -> Self {
        Self {
            zome_index: 0.into(),
            type_index,
        }
    }
}

impl From<LinkType> for ZomeLinkTypesKey {
    fn from(type_index: LinkType) -> Self {
        Self {
            zome_index: 0.into(),
            type_index,
        }
    }
}

#[doc(hidden)]
/// This is an internally used trait for checking
/// enum lengths at compile time.
/// This is used by proc macros in the
/// `hdk_derive` crate and should not be used directly.
pub trait EnumLen {
    /// The total length of an enum (possibly recusively)
    /// known at compile time.
    const ENUM_LEN: u8;
}

#[doc(hidden)]
/// This is an internally used trait for checking
/// enum variant lengths at compile time.
/// This is used by proc macros in the
/// `hdk_derive` crate and should not be used directly.
/// `V` is the variant index.
pub trait EnumVariantLen<const V: u8> {
    /// The starting point of this enum variant.
    const ENUM_VARIANT_START: u8;
    /// The length of this enum variant.
    /// This could include the recusive length of a nested enum.
    const ENUM_VARIANT_INNER_LEN: u8;
    /// The ending point of this variant.
    const ENUM_VARIANT_LEN: u8 = Self::ENUM_VARIANT_START + Self::ENUM_VARIANT_INNER_LEN;
}

/// Helper trait for types that are internally
/// represented as [`u8`] but need to be used
/// as indicies into containers.
pub trait U8Index {
    /// Get the [`usize`] index from this type.
    fn index(&self) -> usize;
}

impl U8Index for ZomeDependencyIndex {
    fn index(&self) -> usize {
        self.0 as usize
    }
}
impl U8Index for EntryDefIndex {
    fn index(&self) -> usize {
        self.0 as usize
    }
}
impl U8Index for LinkType {
    fn index(&self) -> usize {
        self.0 as usize
    }
}

impl From<u8> for ZomeDependencyIndex {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

impl From<()> for ZomeEntryTypesKey {
    fn from(_: ()) -> Self {
        unimplemented!("Should not ever be used")
    }
}
