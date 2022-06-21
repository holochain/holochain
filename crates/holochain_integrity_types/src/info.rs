//! Information about the current zome and dna.
use std::collections::BTreeMap;

use crate::action::ZomeId;
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
///
/// This allows the caller to convert from [`LocalZomeTypeId`] to [`GlobalZomeTypeId`]
/// and back again.
pub struct ScopedZomeTypesSet {
    /// All the entry [`GlobalZomeTypeId`]s in scope for this zome.
    /// Converts from [`EntryDefIndex`](crate::action::EntryDefIndex) to [`LocalZomeTypeId`].
    pub entries: ScopedZomeTypes,
    /// All the link [`GlobalZomeTypeId`]s in scope for this zome.
    /// Converts from [`LinkType`](crate::link::LinkType) to [`LocalZomeTypeId`].
    pub links: ScopedZomeTypes,
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
/// TODO
pub struct ScopedZomeTypes(pub BTreeMap<LocalZomeTypeId, ZomeId>);

impl ScopedZomeTypes {
    /// TODO
    pub fn zome_id(&self, index: impl Into<LocalZomeTypeId>) -> Option<ZomeId> {
        let index = index.into();
        self.0.range(index..).next().map(|(_, z)| *z)
    }

    /// TODO
    pub fn in_scope(&self, index: impl Into<LocalZomeTypeId>, zome_id: impl Into<ZomeId>) -> bool {
        self.zome_id(index).map_or(false, |z| z == zome_id.into())
    }

    /// TODO
    pub fn is_dependency(&self, zome_id: impl Into<ZomeId>) -> bool {
        let zome_id = zome_id.into();
        self.0.iter().any(|(_, z)| *z == zome_id)
    }

    /// TODO
    pub fn all_dependencies(&self) -> Vec<ZomeId> {
        let mut out: Vec<_> = self.0.values().into_iter().copied().collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// TODO
    pub fn offset(
        &self,
        zome_id: impl Into<ZomeId>,
        index: impl Into<LocalZomeTypeId>,
    ) -> Option<LocalZomeTypeId> {
        let zome_id = zome_id.into();
        let index = index.into();
        self.0
            .iter()
            .filter(|(_, z)| **z == zome_id)
            .map(|(l, _)| *l)
            .collect::<Vec<_>>()
            .get(index.0 as usize)
            .copied()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An opaque type identifier that the guest uses to
/// uniquely identify an app defined entry or link type
/// within a single zome.
pub struct LocalZomeTypeId(pub u8);

impl From<u8> for LocalZomeTypeId {
    fn from(v: u8) -> Self {
        Self(v)
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
