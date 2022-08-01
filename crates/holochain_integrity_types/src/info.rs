//! Information about the current zome and dna.
use crate::action::ZomeId;
use crate::zome::ZomeName;
use crate::EntryDefIndex;
use crate::EntryDefs;
use crate::FunctionName;
use crate::LinkType;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

#[cfg(test)]
mod test;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeId,
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
        id: ZomeId,
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

#[derive(Debug, Serialize, Deserialize)]
/// Information about the current DNA.
pub struct DnaInfo {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The properties of this DNA.
    pub properties: SerializedBytes,
    // In ZomeId order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
/// The set of [`EntryDefIndex`] and [`LinkType`]s in scope for the calling zome.
pub struct ScopedZomeTypesSet {
    /// All the entry [`EntryDefIndex`]s in scope for this zome.
    pub entries: ScopedZomeTypes<EntryDefIndex>,
    /// All the entry [`LinkType`]s in scope for this zome.
    pub links: ScopedZomeTypes<LinkType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
/// zome types that are in scope for the calling zome.
pub struct ScopedZomeTypes<T>(pub Vec<(ZomeId, Vec<T>)>);

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
    /// The index into the [`ZomeId`] vec.
    pub zome_index: ZomeDependencyIndex,
    /// The index into the types vec.
    pub type_index: T,
}

/// A key to the [`ScopedZomeTypes<EntryDefIndex>`] container.
pub type ZomeEntryTypesKey = ZomeTypesKey<EntryDefIndex>;
/// A key to the [`ScopedZomeTypes<LinkType>`] container.
pub type ZomeLinkTypesKey = ZomeTypesKey<LinkType>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// The index into the [`ZomeId`] vec.
pub struct ZomeDependencyIndex(pub u8);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A type with the zome that it is defined in.
pub struct ScopedZomeType<T> {
    /// The zome that defines this type.
    pub zome_id: ZomeId,
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
            .and_then(|(zome_id, types)| {
                types
                    .get(key.type_index.index())
                    .copied()
                    .map(|zome_type| ScopedZomeType {
                        zome_id: *zome_id,
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
            .find_map(|key| (self.get(key)? == scoped_type).then(|| key))
    }

    /// Find the [`ZomeTypesKey`] for this [`ScopedZomeType`].
    pub fn find_key(&self, scoped_type: ScopedZomeType<T>) -> Option<ZomeTypesKey<T>>
    where
        T: PartialEq,
        T: From<u8>,
    {
        self.0
            .iter()
            .position(|(zome_id, _)| *zome_id == scoped_type.zome_id)
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

    /// Get all the [`ZomeId`] dependencies for the calling zome.
    pub fn dependencies(&self) -> impl Iterator<Item = ZomeId> + '_ {
        self.0.iter().map(|(zome_id, _)| *zome_id)
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
