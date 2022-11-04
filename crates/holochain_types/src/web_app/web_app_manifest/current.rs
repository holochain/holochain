//! Re-export types from the current version.
//! Simply adjust this import when using a new version.

pub use super::web_app_manifest_v1::{
    WebAppManifestV1 as WebAppManifestCurrent,
    WebAppManifestV1Builder as WebAppManifestCurrentBuilder, *,
};
