//! Implements YamlProperties, and potentially any other data types that can
//! represent "properties" of a DNA

use holochain_serialized_bytes::prelude::*;

/// A type to allow yaml values to be used as [`derive@SerializedBytes`]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct YamlProperties(serde_yaml::Value);

impl YamlProperties {
    /// Create new properties from yaml value
    pub fn new(properties: serde_yaml::Value) -> Self {
        Self(properties)
    }

    /// Create a null set of properties
    pub fn empty() -> Self {
        Self(serde_yaml::Value::Null)
    }

    /// Consumes struct into inner value.
    pub fn into_inner(self) -> serde_yaml::Value {
        self.0
    }
}

impl From<()> for YamlProperties {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

impl From<serde_yaml::Value> for YamlProperties {
    fn from(v: serde_yaml::Value) -> Self {
        Self(v)
    }
}

impl Default for YamlProperties {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for YamlProperties {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "YamlProperties".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
        })
    }
}
