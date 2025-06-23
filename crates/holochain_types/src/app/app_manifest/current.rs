//! Re-export types from the current version.
//! Simply adjust this import when using a new version.

pub use super::app_manifest_v0::{
    AppManifestV0 as AppManifestCurrent, AppManifestV0,
    AppManifestV0Builder as AppManifestCurrentBuilder, AppManifestV0Builder, AppRoleManifest,
};
