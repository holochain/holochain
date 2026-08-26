//! # DNA Properties Support types

use crate::info::NetworkSeed;
use holochain_serialized_bytes::prelude::*;

/// Modifiers of this DNA - the network seed, properties and origin time - as
/// opposed to the actual DNA code. These modifiers are included in the DNA
/// hash computation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "full-dna-def", derive(derive_builder::Builder))]
#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_rs", ts(export_to = "types.ts"))]
pub struct DnaModifiers {
    /// The network seed of a DNA is included in the computation of the DNA hash.
    /// The DNA hash in turn determines the network peers and the DHT, meaning
    /// that only peers with the same DNA hash of a shared DNA participate in the
    /// same network and co-create the DHT. To create a separate DHT for the DNA,
    /// a unique network seed can be specified.
    // TODO: consider Vec<u8> instead (https://github.com/holochain/holochain/pull/86#discussion_r412689085)
    #[cfg_attr(feature = "ts_rs", ts(as = "crate::info::NetworkSeedTs"))]
    pub network_seed: NetworkSeed,

    /// Any arbitrary application properties can be included in this object.
    #[cfg_attr(feature = "full-dna-def", builder(default = "().try_into().unwrap()"))]
    #[cfg_attr(feature = "ts_rs", ts(type = "Uint8Array"))]
    pub properties: SerializedBytes,
}

impl DnaModifiers {
    /// Replace fields in the modifiers with any Some fields in the argument.
    /// None fields remain unchanged.
    pub fn update(mut self, modifiers: DnaModifiersOpt) -> DnaModifiers {
        self.network_seed = modifiers.network_seed.unwrap_or(self.network_seed);
        self.properties = modifiers.properties.unwrap_or(self.properties);
        self
    }
}

/// [`DnaModifiers`] options of which all are optional.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DnaModifiersOpt<P = SerializedBytes> {
    /// see [`DnaModifiers`]
    pub network_seed: Option<NetworkSeed>,
    /// see [`DnaModifiers`]
    #[cfg_attr(feature = "schema", schemars(schema_with = "properties_schema"))]
    pub properties: Option<P>,
}

impl<P> DnaModifiersOpt<P> {
    /// Replaces fields with any `Some` fields from `modifiers`.
    pub fn update(mut self, modifiers: Self) -> Self {
        self.network_seed = modifiers.network_seed.or(self.network_seed);
        self.properties = modifiers.properties.or(self.properties);
        self
    }
}

/// Supplies the TypeScript shape for `DnaModifiersOpt<P>::properties` at a
/// given concrete `P`.
///
/// `DnaModifiersOpt<P>` can't derive `ts_rs::TS` directly: its default `P`
/// is `SerializedBytes`, foreign to this crate and thus unable to implement
/// `ts_rs::TS` here (orphan rules), and pinning the whole type to one
/// concrete `P` via `#[ts(concrete(P = ..))]` would break other
/// instantiations (e.g. `holochain_zome_types::properties::YamlProperties`)
/// that need their own shape for `properties`.
///
/// `DnaModifiersOpt<P>`'s manual `TS` impl is bounded on this crate-local
/// trait instead, so foreign `SerializedBytes` can still implement it. Every
/// concrete `P` needs one impl, giving each instantiation its own
/// `properties` shape.
#[cfg(feature = "ts_rs")]
pub trait DnaPropertiesTs {
    /// The TypeScript type name to substitute for `P` in
    /// `DnaModifiersOpt<P>::properties`.
    fn dna_properties_ts_name(cfg: &ts_rs::Config) -> String;

    /// Register whatever `ts_rs` dependency this `P` needs, so its import is
    /// emitted whenever `DnaModifiersOpt<P>` is exported.
    fn visit_dna_properties_ts_dependency(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static;
}

#[cfg(feature = "ts_rs")]
impl DnaPropertiesTs for SerializedBytes {
    fn dna_properties_ts_name(_: &ts_rs::Config) -> String {
        "Uint8Array".into()
    }

    fn visit_dna_properties_ts_dependency(_: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
    }
}

#[cfg(feature = "ts_rs")]
impl<P> ts_rs::TS for DnaModifiersOpt<P>
where
    P: DnaPropertiesTs,
{
    type WithoutGenerics = DnaModifiersOptWithoutGenerics;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        format!("DnaModifiersOpt<{}>", P::dna_properties_ts_name(cfg))
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        format!(
            "{{ network_seed?: {} | null, properties?: {} | null }}",
            <crate::info::NetworkSeedTs as ts_rs::TS>::name(cfg),
            P::dna_properties_ts_name(cfg)
        )
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type DnaModifiersOpt<P> = { network_seed?: NetworkSeed | null, properties?: P | null };"
            .into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        format!(
            "type DnaModifiersOpt = {};",
            <Self as ts_rs::TS>::inline(cfg)
        )
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<crate::info::NetworkSeedTs>();
        P::visit_dna_properties_ts_dependency(v);
    }

    // A struct embedding `DnaModifiersOpt<YamlProperties>` renders the field
    // as `DnaModifiersOpt<YamlProperties>` but won't import `YamlProperties`
    // unless it's registered here too (same `visit_generics` pattern as
    // `Signed<T>` in `holochain_zome_types::signature`, routed through
    // `DnaPropertiesTs` since `P` isn't bounded on `TS` directly).
    fn visit_generics(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        P::visit_dna_properties_ts_dependency(v);
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("types.ts".into())
    }
}

/// [`DnaModifiersOpt`]'s [`ts_rs::TS::WithoutGenerics`] — just the imports
/// its fixed declaration text references (`NetworkSeed`), unrelated to any
/// concrete `P`.
#[cfg(feature = "ts_rs")]
#[doc(hidden)]
pub struct DnaModifiersOptWithoutGenerics;

#[cfg(feature = "ts_rs")]
impl ts_rs::TS for DnaModifiersOptWithoutGenerics {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "DnaModifiersOpt".into()
    }

    fn inline(_: &ts_rs::Config) -> String {
        panic!("DnaModifiersOptWithoutGenerics is a type-level placeholder and cannot be inlined")
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<crate::info::NetworkSeedTs>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("types.ts".into())
    }
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_export {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn properties_shape_is_per_instantiation() {
        let cfg = ts_rs::Config::default();

        assert_eq!(
            DnaModifiersOpt::<SerializedBytes>::inline(&cfg),
            "{ network_seed?: NetworkSeed | null, properties?: Uint8Array | null }"
        );
    }
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> Default
    for DnaModifiersOpt<P>
{
    fn default() -> Self {
        Self::none()
    }
}

impl<P: TryInto<SerializedBytes, Error = E>, E: Into<SerializedBytesError>> DnaModifiersOpt<P> {
    /// Constructor with all fields set to `None`
    pub fn none() -> Self {
        Self {
            network_seed: None,
            properties: None,
        }
    }

    /// Serialize the properties field into SerializedBytes
    pub fn serialized(self) -> Result<DnaModifiersOpt<SerializedBytes>, E> {
        let Self {
            network_seed,
            properties,
        } = self;
        let properties = if let Some(p) = properties {
            Some(p.try_into()?)
        } else {
            None
        };
        Ok(DnaModifiersOpt {
            network_seed,
            properties,
        })
    }

    /// Return a modified form with the `network_seed` field set
    pub fn with_network_seed(mut self, network_seed: NetworkSeed) -> Self {
        self.network_seed = Some(network_seed);
        self
    }

    /// Return a modified form with the `properties` field set
    pub fn with_properties(mut self, properties: P) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Check if at least one of the options is set.
    pub fn has_some_option_set(&self) -> bool {
        self.network_seed.is_some() || self.properties.is_some()
    }
}

/// Trait to convert from dna properties into specified type
pub trait TryFromDnaProperties {
    /// The error associated with this conversion.
    type Error;

    /// Attempts to deserialize DNA properties into specified type
    fn try_from_dna_properties() -> Result<Self, Self::Error>
    where
        Self: Sized;
}

#[cfg(feature = "schema")]
fn properties_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": ["object", "null"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_modifiers_update_only_set_fields() {
        let original = DnaModifiersOpt {
            network_seed: Some("original-seed".to_string()),
            properties: Some("original-properties".to_string()),
        };
        let overrides = DnaModifiersOpt {
            network_seed: Some("override-seed".to_string()),
            properties: None,
        };

        let updated = original.update(overrides);

        assert_eq!(updated.network_seed.as_deref(), Some("override-seed"));
        assert_eq!(updated.properties.as_deref(), Some("original-properties"));
    }
}
