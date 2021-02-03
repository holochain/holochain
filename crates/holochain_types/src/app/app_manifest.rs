#![warn(missing_docs)]

//! Defines the hApp Manifest YAML format, including validation.

use mr_bundle::{Location, Manifest};
use serde;
use std::path::PathBuf;

pub(crate) mod app_manifest_v1;
pub mod app_manifest_validated;
mod current;
mod error;

pub use current::*;
pub use error::*;

use self::{app_manifest_validated::AppManifestValidated, error::AppManifestResult};

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

impl AppManifest {
    /// Convert this human-focused manifest into a validated, concise representation
    pub fn validate(self) -> AppManifestResult<AppManifestValidated> {
        match self {
            Self::V1(manifest) => manifest.validate(),
        }
    }
}
