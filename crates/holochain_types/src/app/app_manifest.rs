#![warn(missing_docs)]

//! Defines the hApp Manifest YAML format, including validation.

use mr_bundle::{Location, Manifest};
use serde;
use std::path::PathBuf;

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

impl Manifest for AppManifest {
    fn locations(&self) -> Vec<Location> {
        match self {
            AppManifest::V1(app_manifest_v1::AppManifestV1 { cells, .. }) => cells
                .iter()
                .filter_map(|cell| cell.dna.location.clone())
                .collect(),
        }
    }

    fn path(&self) -> PathBuf {
        "app.yaml".into()
    }
}
