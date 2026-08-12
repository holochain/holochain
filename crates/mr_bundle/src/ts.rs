//! Hand-written [`ts_rs::TS`] impls for `mr_bundle`'s Rust type aliases.
//!
//! [`crate::ResourceIdentifier`] and [`crate::ResourceMap`] are plain `type`
//! aliases, not distinct types, so `ts-rs` can't derive bindings for them.
//! Each is modeled as a marker type whose [`TS`] impl emits the alias
//! declaration, mirroring the `ts_alias!` macro in `holo_hash::ts` without
//! taking on that dependency — `mr_bundle` stays reusable outside Holochain.

use ts_rs::TS;

/// Marker type emitting the `ResourceIdentifier` TypeScript declaration.
#[doc(hidden)]
pub struct ResourceIdentifierTs;

impl TS for ResourceIdentifierTs {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "ResourceIdentifier".into()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type ResourceIdentifier = string;".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        Self::decl(cfg)
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("api/admin/types.ts".into())
    }
}

/// Marker type emitting the `ResourceMap` TypeScript declaration.
#[doc(hidden)]
pub struct ResourceMapTs;

impl TS for ResourceMapTs {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "ResourceMap".into()
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        Self::name(cfg)
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type ResourceMap = Record<ResourceIdentifier, ResourceBytes>;".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        Self::decl(cfg)
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<ResourceIdentifierTs>();
        v.visit::<crate::ResourceBytes>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("api/admin/types.ts".into())
    }
}

/// [`crate::Bundle`]'s [`ts_rs::TS::WithoutGenerics`] — just the imports its
/// fixed declaration text references (`ResourceMap`), unrelated to any
/// concrete `M`. See the `TS` impl in `crate::bundle`.
#[doc(hidden)]
pub struct BundleWithoutGenerics;

impl TS for BundleWithoutGenerics {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &ts_rs::Config) -> String {
        "Bundle".into()
    }

    fn inline(_: &ts_rs::Config) -> String {
        panic!("BundleWithoutGenerics is a type-level placeholder and cannot be inlined")
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<ResourceMapTs>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("api/admin/types.ts".into())
    }
}

/// Exports every TypeScript declaration this crate contributes to the
/// conductor API bindings.
///
/// Called by downstream crates' `export_ts_bindings` so the whole binding
/// tree is written by a single process.
pub fn export_ts_bindings(cfg: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    ResourceIdentifierTs::export_all(cfg)?;
    ResourceMapTs::export_all(cfg)?;
    Ok(())
}
