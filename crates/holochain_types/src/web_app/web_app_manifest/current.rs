//! Re-export types from the current version.
//! Simply adjust this import when using a new version.

pub use super::web_app_manifest_v0::{
    WebAppManifestV0 as WebAppManifestCurrent,
    WebAppManifestV0Builder as WebAppManifestCurrentBuilder, *,
};
