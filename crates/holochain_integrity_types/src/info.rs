#![warn(missing_docs)]
//! Information about the current zome and dna.
use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Range;

use crate::header::ZomeId;
use crate::zome::ZomeName;
use crate::AppEntryDefName;
use crate::AppEntryType;
use crate::EntryDefId;
use crate::EntryDefIndex;
use crate::EntryDefs;
use crate::FunctionName;
use crate::LinkType;
use crate::LinkTypeName;
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
    /// Zome types in scope for this zome.
    pub zome_types: ScopedZomeTypes,
}

impl ZomeInfo {
    /// Create a new ZomeInfo.
    pub fn new(
        name: ZomeName,
        id: ZomeId,
        properties: SerializedBytes,
        entry_defs: EntryDefs,
        extern_fns: Vec<FunctionName>,
        zome_types: ScopedZomeTypes,
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

    /// Check if an [`AppEntryType`] matches the [`EntryDefId`] provided for this zome.
    pub fn matches_entry_def_id(&self, entry_type: &AppEntryType, id: EntryDefId) -> bool {
        self.entry_defs
            .0
            .get(entry_type.id.index())
            .map_or(false, |stored_id| stored_id.id == id)
            && self.id == entry_type.zome_id
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

// #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
// /// Mapping from [`ZomeId`] to
// /// - Entries: [`AppEntryDefName`] -> [`EntryDefIndex`].
// /// - Links: [`LinkTypeName`] -> [`LinkType`].
// pub struct ZomeTypesMap(pub HashMap<ZomeId, ZomeTypes>);

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
/// A set of zome types with their name space paths.
pub struct ScopedZomeTypes {
    pub entries: ScopedZomeType,
    pub links: ScopedZomeType,
}

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Default)]
pub struct ScopedZomeType(pub Vec<Range<GlobalZomeTypeId>>);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An opaque type identifier that the guest uses to
/// uniquely identify an app defined entry or link type.
pub struct GlobalZomeTypeId(pub u8);

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// An opaque type identifier that the guest uses to
/// uniquely identify an app defined entry or link type.
pub struct LocalZomeTypeId(pub u8);

impl ScopedZomeType {
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
// #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
// /// Entry and link types for a zome.
// pub struct ZomeTypes {
//     /// Map of [`AppEntryDefName`] to [`EntryDefIndex`].
//     pub entry: HashMap<AppEntryDefName, EntryDefIndex>,
//     /// Map of [`LinkTypeName`] to [`LinkType`].
//     pub link: HashMap<LinkTypeName, LinkType>,
// }

// impl std::ops::Deref for ZomeTypesMap {
//     type Target = HashMap<ZomeId, ZomeTypes>;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl ZomeTypesMap {
//     /// Get the [`EntryDefIndex`] for a [`ZomeId`] and [`AppEntryDefName`]
//     /// if there is one.
//     pub fn get_entry_index<Z, E>(&self, zome_id: &Z, entry_name: &E) -> Option<EntryDefIndex>
//     where
//         ZomeId: Borrow<Z>,
//         Z: std::hash::Hash + Eq,
//         AppEntryDefName: Borrow<E>,
//         E: std::hash::Hash + Eq + ?Sized,
//     {
//         self.get(zome_id)
//             .and_then(|z| z.entry.get(entry_name).copied())
//     }

//     /// Get the [`LinkType`] for a [`ZomeId`] and [`LinkTypeName`]
//     /// if there is one.
//     pub fn get_link_type<Z, L>(&self, zome_id: &Z, link_name: &L) -> Option<LinkType>
//     where
//         ZomeId: Borrow<Z>,
//         Z: std::hash::Hash + Eq,
//         LinkTypeName: Borrow<L>,
//         L: std::hash::Hash + Eq,
//     {
//         self.get(zome_id)
//             .and_then(|z| z.link.get(link_name).copied())
//     }

//     /// Find the [`ZomeId`] for a given [`EntryDefIndex`].
//     pub fn entry_index_to_zome_id(&self, index: &EntryDefIndex) -> Option<ZomeId> {
//         self.iter()
//             .find_map(|(id, zome_types)| zome_types.entry.values().any(|i| i == index).then(|| *id))
//     }

//     /// Find the [`ZomeId`] for a given [`LinkType`].
//     pub fn link_type_to_zome_id<I>(&self, index: &LinkType) -> Option<ZomeId> {
//         self.iter()
//             .find_map(|(id, zome_types)| zome_types.link.values().any(|i| i == index).then(|| *id))
//     }
// }

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
