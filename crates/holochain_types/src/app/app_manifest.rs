//! Defines the DNA Bundle manifest format.
#![warn(missing_docs)]

use serde;

mod app_manifest_v1;
mod app_manifest_validated;
mod current;
mod error;

/// Alias for the current AppManifest format
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum AppManifest {
    #[serde(rename = "1")]
    #[serde(alias = "\"1\"")]
    V1(app_manifest_v1::AppManifestV1),
}
