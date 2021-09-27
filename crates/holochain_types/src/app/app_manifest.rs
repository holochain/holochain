#![warn(missing_docs)]

//! Defines the hApp Manifest YAML format, including validation.

use holochain_zome_types::Uid;
use mr_bundle::{Location, Manifest};
use std::path::PathBuf;

pub(crate) mod app_manifest_v1;
pub mod app_manifest_validated;
mod current;
mod error;

pub use current::*;
pub use error::*;

use self::{app_manifest_validated::AppManifestValidated, error::AppManifestResult};
use app_manifest_v1::AppManifestV1;

/// Container struct which uses the `manifest_version` field to determine
/// which manifest version to deserialize to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum AppManifest {
    #[serde(rename = "1")]
    V1(AppManifestV1),
}

impl Manifest for AppManifest {
    fn locations(&self) -> Vec<Location> {
        match self {
            AppManifest::V1(m) => m
                .slots
                .iter()
                .filter_map(|slot| slot.dna.location.clone())
                .collect(),
        }
    }

    fn path() -> PathBuf {
        "happ.yaml".into()
    }

    fn bundle_extension() -> &'static str {
        "happ"
    }
}

impl AppManifest {
    /// Get the supplied name of the app
    pub fn app_name(&self) -> &str {
        match self {
            Self::V1(AppManifestV1 { name, .. }) => name,
        }
    }

    /// Convert this human-focused manifest into a validated, concise representation
    pub fn validate(self) -> AppManifestResult<AppManifestValidated> {
        match self {
            Self::V1(manifest) => manifest.validate(),
        }
    }

    /// Update the UID for all DNAs used in Create-provisioned Cells.
    /// Cells with other provisioning strategies are not affected.
    pub fn set_uid(&mut self, uid: Uid) {
        match self {
            Self::V1(manifest) => manifest.set_uid(uid),
        }
    }

    /// Returns the list of app slots that this manifest declares
    pub fn app_slots(&self) -> Vec<AppSlotManifest> {
        match self {
            Self::V1(manifest) => manifest.slots.clone(),
        }
    }
}

#[cfg(test)]
pub mod tests {

    use mr_bundle::Manifest;

    use crate::app::app_manifest::{AppManifest, AppManifestV1Builder, AppSlotManifest};

    #[test]
    /// Replicate this test for any new version of the manifest that gets created
    fn app_manifest_v1_helper_functions() {
        let app_name = String::from("sample-app");

        let dna_slot = String::from("sample-dna");
        let app_slot_manifest = AppSlotManifest::sample(dna_slot);

        let sample_app_manifest_v1 = AppManifestV1Builder::default()
            .name(app_name.clone())
            .description(Some(String::from("Some description")))
            .slots(vec![app_slot_manifest.clone()])
            .build()
            .unwrap();
        let sample_app_manifest = AppManifest::V1(sample_app_manifest_v1.clone());

        assert_eq!(app_name, sample_app_manifest.app_name());
        assert_eq!(vec![app_slot_manifest], sample_app_manifest.app_slots());
        assert_eq!(
            vec![sample_app_manifest_v1
                .slots
                .get(0)
                .unwrap()
                .dna
                .location
                .clone()
                .unwrap()],
            sample_app_manifest.locations()
        );
    }
}
