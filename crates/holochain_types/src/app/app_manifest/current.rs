//! Re-export types from the current version.
//! Simply adjust this import when using a new version.

pub use super::app_manifest_v1::{
    AppManifestV1 as AppManifestInner, AppManifestV1Builder as AppManifestInnerBuilder, *,
};
