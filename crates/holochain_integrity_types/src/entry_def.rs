use std::borrow::Borrow;
use std::borrow::Cow;

use holochain_serialized_bytes::prelude::*;

const DEFAULT_REQUIRED_VALIDATIONS: u8 = 5;

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum EntryDefId {
    App(AppEntryName),
    CapClaim,
    CapGrant,
}

/// Identifier for an entry definition.
/// This may be removed.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AppEntryName(pub Cow<'static, str>);

#[cfg(feature = "fuzzing")]
impl<'a> arbitrary::Arbitrary<'a> for AppEntryName {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self(Cow::Owned(String::arbitrary(u)?)))
    }
}

/// Trait for binding static [`EntryDef`] property access for a type.
/// This trait maps a type to its corresponding [`EntryDef`] property
/// at compile time.
///
/// # Derivable
/// This trait can be used with `#[derive]` or by using the attribute macro `hdk_derive::hdk_entry_types`.
pub trait EntryDefRegistration {
    /// The list of [`EntryDef`] properties for the implementing type.
    /// This must be in the same order as the
    const ENTRY_DEFS: &'static [EntryDef];
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
/// The number of validations required for an entry to
/// be considered published.
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct RequiredValidations(pub u8);

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct EntryDef {
    /// Zome-unique identifier for this entry type
    pub id: EntryDefId,
    /// Public or Private
    pub visibility: EntryVisibility,
    /// how many validations to receive before considered "network saturated" (MAX value of 50?)
    pub required_validations: RequiredValidations,
    /// Should this entry be cached with agent activity authorities
    /// for reduced networked hops when using `must_get_agent_activity`.
    /// Note this will result in more storage being used on the DHT.
    /// Defaults to false.
    pub cache_at_agent_activity: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// All definitions for all entry types in an integrity zome.
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
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum EntryVisibility {
    Public,
    Private,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum EntryDefsCallbackResult {
    Defs(EntryDefs),
}

impl AppEntryName {
    /// Create a new [`AppEntryName`] from a string or `&'static str`.
    pub fn new(s: impl Into<Cow<'static, str>>) -> Self {
        Self(s.into())
    }
    /// Create a new [`AppEntryName`] from a `&'static str`.
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
        cache_at_agent_activity: bool,
    ) -> Self {
        Self {
            id,
            visibility,
            required_validations,
            cache_at_agent_activity,
        }
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn default_from_id<I: Into<EntryDefId>>(id: I) -> Self {
        EntryDef {
            id: id.into(),
            ..Default::default()
        }
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

impl From<&'static str> for AppEntryName {
    fn from(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl From<String> for AppEntryName {
    fn from(s: String) -> Self {
        Self(Cow::Owned(s))
    }
}

impl From<AppEntryName> for EntryDefId {
    fn from(name: AppEntryName) -> Self {
        EntryDefId::App(name)
    }
}

impl Borrow<str> for AppEntryName {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl std::fmt::Display for AppEntryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(any(test, feature = "test_utils"))]
impl Default for EntryDef {
    fn default() -> Self {
        Self {
            id: EntryDefId::App(AppEntryName(Default::default())),
            visibility: Default::default(),
            required_validations: Default::default(),
            cache_at_agent_activity: false,
        }
    }
}
