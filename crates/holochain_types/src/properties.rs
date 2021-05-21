//! Implements YamlProperties, and potentially any other data types that can
//! represent "properties" of a DNA

use holochain_serialized_bytes::prelude::*;

/// A type to allow json values to be used as [SerializedBytes]
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    derive_more::From,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct YamlProperties(serde_yaml::Value);

impl YamlProperties {
    /// Create new properties from json value
    pub fn new(properties: serde_yaml::Value) -> Self {
        Self(properties)
    }

    /// Create a null set of properties
    pub fn empty() -> Self {
        Self(serde_yaml::Value::Null)
    }
}

impl From<()> for YamlProperties {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

impl Default for YamlProperties {
    fn default() -> Self {
        Self::empty()
    }
}

/// Not a great implementation: always returns null
#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for YamlProperties {
    fn arbitrary(_: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(serde_yaml::Value::Null.into())
    }
}
