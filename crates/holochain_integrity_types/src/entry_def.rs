use std::borrow::Cow;

use crate::validate::RequiredValidationType;
use crate::EntryDefIndex;
use holochain_serialized_bytes::prelude::*;

const DEFAULT_REQUIRED_VALIDATIONS: u8 = 5;

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum EntryDefId {
    App(AppEntryDefName),
    CapClaim,
    CapGrant,
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct AppEntryDefName(pub Cow<'static, str>);

/// A trait for converting a value to a [`AppEntryDefName`].
pub trait ToAppEntryDefName {
    /// Converts the value to a [`AppEntryDefName`] without consuming it.
    fn entry_def_name(&self) -> AppEntryDefName;
}

/// Trait for binding static [`EntryDef`] property access for a type.
/// See [`register_entry`]
pub trait EntryDefRegistration {
    const ENTRY_DEFS: &'static [AppEntryDef];
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct RequiredValidations(pub u8);

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct EntryDef {
    /// Zome-unique identifier for this entry type
    pub id: EntryDefId,
    /// Public or Private
    pub visibility: EntryVisibility,
    /// how many validations to receive before considered "network saturated" (MAX value of 50?)
    pub required_validations: RequiredValidations,
    /// The required validation package for this entry
    pub required_validation_type: RequiredValidationType,
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct AppEntryDef {
    /// Zome-unique identifier for this entry type
    pub name: AppEntryDefName,
    /// Public or Private
    pub visibility: EntryVisibility,
    /// how many validations to receive before considered "network saturated" (MAX value of 50?)
    pub required_validations: RequiredValidations,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryDefs(pub Vec<EntryDef>);

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    SerializedBytes,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum EntryVisibility {
    Public,
    Private,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum EntryDefsCallbackResult {
    Defs(EntryDefs),
}

impl AppEntryDefName {
    pub fn new(s: impl Into<Cow<'static, str>>) -> Self {
        Self(s.into())
    }
    pub const fn from_str(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl Default for EntryVisibility {
    fn default() -> Self {
        Self::Public
    }
}

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

impl EntryDef {
    pub fn new(
        id: EntryDefId,
        visibility: EntryVisibility,
        required_validations: RequiredValidations,
        required_validation_type: RequiredValidationType,
    ) -> Self {
        Self {
            id,
            visibility,
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
        )
    }
}

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

impl From<Vec<EntryDef>> for EntryDefsCallbackResult {
    fn from(v: Vec<EntryDef>) -> Self {
        Self::Defs(v.into())
    }
}

impl From<String> for EntryDefId {
    fn from(s: String) -> Self {
        Self::App(s.into())
    }
}

impl From<&str> for EntryDefId {
    fn from(s: &str) -> Self {
        Self::App(s.to_string().into())
    }
}

impl From<&'static str> for AppEntryDefName {
    fn from(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl From<String> for AppEntryDefName {
    fn from(s: String) -> Self {
        Self(Cow::Owned(s))
    }
}

impl From<AppEntryDefName> for EntryDefId {
    fn from(name: AppEntryDefName) -> Self {
        EntryDefId::App(name)
    }
}

impl From<AppEntryDef> for EntryDef {
    fn from(app: AppEntryDef) -> Self {
        Self {
            id: app.name.into(),
            visibility: app.visibility,
            required_validations: app.required_validations,
            required_validation_type: Default::default(),
        }
    }
}
