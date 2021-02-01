//! Defines the hApp Manifest YAML format, including validation.

#![warn(missing_docs)]

mod app_manifest_v1;
mod app_manifest_validated;
mod current;
mod error;

/// Container struct which uses the `manifest_version` field to determine
/// which manifest version to deserialize to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum AppManifest {
    #[serde(rename = "1")]
    #[serde(alias = "\"1\"")]
    V1(app_manifest_v1::AppManifestV1),
}
