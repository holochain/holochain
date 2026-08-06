//! Implements YamlProperties, and potentially any other data types that can
//! represent "properties" of a DNA

use holochain_serialized_bytes::prelude::*;

/// A type to allow yaml values to be used as [`derive@SerializedBytes`]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export, export_to = "types.ts"))]
pub struct YamlProperties(#[cfg_attr(feature = "ts_rs", ts(type = "unknown"))] yaml_serde::Value);

impl YamlProperties {
    /// Create new properties from yaml value
    pub fn new(properties: yaml_serde::Value) -> Self {
        Self(properties)
    }

    /// Create a null set of properties
    pub fn empty() -> Self {
        Self(yaml_serde::Value::Null)
    }

    /// Consumes struct into inner value.
    pub fn into_inner(self) -> yaml_serde::Value {
        self.0
    }
}

impl From<()> for YamlProperties {
    fn from(_: ()) -> Self {
        Self::empty()
    }
}

impl From<yaml_serde::Value> for YamlProperties {
    fn from(v: yaml_serde::Value) -> Self {
        Self(v)
    }
}

impl Default for YamlProperties {
    fn default() -> Self {
        Self::empty()
    }
}

// `DnaModifiersOpt<P>`'s `TS` impl bounds on `DnaPropertiesTs` rather than
// `ts_rs::TS` directly (see its doc comment), so `YamlProperties` implements
// that instead.
#[cfg(feature = "ts_rs")]
impl holochain_integrity_types::prelude::DnaPropertiesTs for YamlProperties {
    fn dna_properties_ts_name(cfg: &ts_rs::Config) -> String {
        <YamlProperties as ts_rs::TS>::name(cfg)
    }

    fn visit_dna_properties_ts_dependency(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<YamlProperties>();
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
