use crate::crdt::CrdtType;
use crate::validate::RequiredValidationType;
use crate::CallbackResult;
use crate::EntryDefIndex;
use holochain_serialized_bytes::prelude::*;
use holochain_wasmer_common::WasmError;

const DEFAULT_REQUIRED_VALIDATIONS: u8 = 5;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EntryDefId {
    App(String),
    CapClaim,
    CapGrant,
}

impl From<String> for EntryDefId {
    fn from(s: String) -> Self {
        Self::App(s)
    }
}

impl From<&str> for EntryDefId {
    fn from(s: &str) -> Self {
        Self::App(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Copy, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum EntryVisibility {
    Public,
    Private,
}

impl Default for EntryVisibility {
    fn default() -> Self {
        Self::Public
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RequiredValidations(u8);

impl From<u8> for RequiredValidations {
    fn from(u: u8) -> Self {
        Self(u)
    }
}

impl From<RequiredValidations> for u8 {
    fn from(required_validations: RequiredValidations) -> Self {
        required_validations.0
    }
}

impl Default for RequiredValidations {
    fn default() -> Self {
        Self(DEFAULT_REQUIRED_VALIDATIONS)
    }
}

impl EntryVisibility {
    /// converts entry visibility enum into boolean value on public
    pub fn is_public(&self) -> bool {
        *self == EntryVisibility::Public
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDef {
    /// Zome-unique identifier for this entry type
    pub id: EntryDefId,
    /// Public or Private
    pub visibility: EntryVisibility,
    /// TBD -- Special types of conflict resolution support from Holochain (e.g. Single-Author, )
    pub crdt_type: CrdtType,
    /// how many validations to receive before considered "network saturated" (MAX value of 50?)
    pub required_validations: RequiredValidations,
    /// The required validation package for this entry
    pub required_validation_type: RequiredValidationType,
}

impl EntryDef {
    pub fn new(
        id: EntryDefId,
        visibility: EntryVisibility,
        crdt_type: CrdtType,
        required_validations: RequiredValidations,
        required_validation_type: RequiredValidationType,
    ) -> Self {
        Self {
            id,
            visibility,
            crdt_type,
            required_validations,
            required_validation_type,
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn default_with_id<I: Into<EntryDefId>>(id: I) -> Self {
        EntryDef::new(
            id.into(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDefs(Vec<EntryDef>);

impl EntryDefs {
    pub fn entry_def_index_from_id(&self, entry_def_id: EntryDefId) -> Option<EntryDefIndex> {
        self.0
            .iter()
            .position(|entry_def| entry_def.id == entry_def_id)
            .map(|u_size| EntryDefIndex(u_size as u8))
    }
}

impl std::ops::Index<usize> for EntryDefs {
    type Output = EntryDef;
    fn index(&self, i: usize) -> &Self::Output {
        &self.0[i]
    }
}

impl IntoIterator for EntryDefs {
    type Item = EntryDef;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Vec<EntryDef>> for EntryDefs {
    fn from(v: Vec<EntryDef>) -> Self {
        Self(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum EntryDefsCallbackResult {
    Defs(EntryDefs),
}

impl From<Vec<EntryDef>> for EntryDefsCallbackResult {
    fn from(v: Vec<EntryDef>) -> Self {
        Self::Defs(v.into())
    }
}

impl CallbackResult for EntryDefsCallbackResult {
    fn is_definitive(&self) -> bool {
        false
    }
    fn try_from_wasm_error(wasm_error: WasmError) -> Result<Self, WasmError> {
        // There is no concept of entry defs failing, other than normal error handling.
        Err(wasm_error)
    }
}
