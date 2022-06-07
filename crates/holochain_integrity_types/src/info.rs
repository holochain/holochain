//! Information about the current zome and dna.
use core::ops::Range;

use crate::header::ZomeId;
use crate::zome::ZomeName;
use crate::EntryDefs;
use crate::FunctionName;
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
/// The set of entry and link [`GlobalZomeTypeId`]s in scope for the calling zome.
pub struct ScopedZomeTypesSet {
    /// All the entry [`GlobalZomeTypeId`]s in scope for this zome.
    pub entries: ScopedZomeTypes,
    /// All the link [`GlobalZomeTypeId`]s in scope for this zome.
    pub links: ScopedZomeTypes,
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
/// A set of [`GlobalZomeTypeId`] ranges that are in scope for the calling zome.
pub struct ScopedZomeTypes(pub Vec<Range<GlobalZomeTypeId>>);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An opaque type identifier that the guest uses to
/// uniquely identify an app defined entry or link type.
pub struct GlobalZomeTypeId(pub u8);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An opaque type identifier that the guest uses to
/// uniquely identify an app defined entry or link type.
pub struct LocalZomeTypeId(pub u8);

impl ScopedZomeTypes {
    /// Convert a [`GlobalZomeTypeId`] to a [`LocalZomeTypeId`].
    pub fn to_local_scope(&self, index: impl Into<GlobalZomeTypeId>) -> Option<LocalZomeTypeId> {
        let index = index.into();
        let mut total_len: u8 = 0;
        self.0.iter().find_map(|range| {
            total_len = total_len.checked_add(range.end.0.checked_sub(range.start.0)?)?;
            range
                .contains(&index)
                .then(|| (range.end.0 as i16) - (total_len as i16))
                .and_then(|offset| {
                    let i = (index.0 as i16) - offset;
                    Some(LocalZomeTypeId(u8::try_from(i).ok()?))
                })
        })
    }
    /// Convert a [`LocalZomeTypeId`] to a [`GlobalZomeTypeId`].
    pub fn to_global_scope(&self, index: impl Into<LocalZomeTypeId>) -> Option<GlobalZomeTypeId> {
        let index: LocalZomeTypeId = index.into();
        let mut total_len: u8 = 0;
        self.0.iter().find_map(|range| {
            total_len = total_len.checked_add(range.end.0.checked_sub(range.start.0)?)?;
            (index.0 < total_len)
                .then(|| (range.end.0 as i16) - (total_len as i16))
                .and_then(|offset| {
                    let i = (index.0 as i16) + offset;

                    Some(GlobalZomeTypeId(u8::try_from(i).ok()?))
                })
        })
    }
}

impl From<u8> for GlobalZomeTypeId {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

impl From<u8> for LocalZomeTypeId {
    fn from(v: u8) -> Self {
        Self(v)
    }
}

#[doc(hidden)]
/// This is an internally used trait for checking
/// enum lengths at compile time.
pub trait EnumLen<const L: u8> {
    const ENUM_LEN: u8 = L;
}

#[doc(hidden)]
/// This is an internally used trait for checking
/// enum variant lengths at compile time.
pub trait EnumVariantLen<const V: u8> {
    const ENUM_VARIANT_START: u8;
    const ENUM_VARIANT_INNER_LEN: u8;
    const ENUM_VARIANT_LEN: u8 = Self::ENUM_VARIANT_START + Self::ENUM_VARIANT_INNER_LEN;
}
