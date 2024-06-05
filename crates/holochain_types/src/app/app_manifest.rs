#![warn(missing_docs)]

//! The App Manifest format.
//!
//! A running Holochain App (hApp) consists of a collection of Cells (instances
//! of DNA), and these Cells may be shared amongst different apps, enabling
//! inter-app communication. Therefore, in order to install an App, there needs
//! to be a precise specification of what kinds of Cells that App needs available
//! in order to function properly. Such a specification must include info such as:
//! - the acceptable DNA versions that a Cell may use (made possible via DNA
//!   Migrations, which are not yet implemented)
//! - whether a given Cell should be created fresh, or an existing Cell be
//!   borrowed from an already-installed app
//! - whether the app can create cloned copies of a Cell
//!
//! The App Manifest is such a specification. Rather than specify a fixed list
//! of Cells (which would be impossible because each user will be using different
//! Agents and potentially even different versions of a DNA), the manifest
//! is mainly defined by a collection of "roles",
//! each of which may be populated with Cells (instances of DNA) either during
//! app installation or during runtime. Aside from the role definitions, an
//! app also has a `name`, which is used as the `installed_app_id` and must be
//! globally unique, as well as a `description`, which is intended for humans only.
//!
//! Each Role definition specifies what kind of Cell can occupy it.
//! You can think of a Role as a declaration of some piece of functionality
//! that an app needs in order to function, which will be provided by some Cell
//! in a flexible manner depending on the state of the conductor at the time of
//! installation.
//!
//! Each Role definition is made up of:
//! - a RoleName, which only needs to be unique within this App
//! - a provisioning strategy, [`CellProvisioning`], which describes if and how a Cell
//!   should be created freshly for this app, or whether an existing Cell should
//!   occupy this role
//! - a DNA descriptor, [`AppRoleDnaManifest`], which describes where to find the DNA,
//!   the acceptable range of versions, and the cloning limitations.

use holochain_zome_types::prelude::*;
use mr_bundle::{Location, Manifest};
use std::path::PathBuf;

pub(crate) mod app_manifest_v1;
pub mod app_manifest_validated;
mod current;
mod error;

pub use app_manifest_v1::{AppRoleDnaManifest, CellProvisioning};
pub use current::*;
pub use error::*;

use self::app_manifest_validated::AppManifestValidated;

use super::InstalledCell;

/// Container struct which uses the `manifest_version` field to determine
/// which manifest version to deserialize to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
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
                .roles
                .iter()
                .filter_map(|role| role.dna.location.clone())
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

    /// Update the network seed for all DNAs used in Create-provisioned Cells.
    /// Cells with other provisioning strategies are not affected.
    pub fn set_network_seed(&mut self, network_seed: NetworkSeed) {
        match self {
            Self::V1(manifest) => manifest.set_network_seed(network_seed),
        }
    }

    /// Returns the list of app roles that this manifest declares
    pub fn app_roles(&self) -> Vec<AppRoleManifest> {
        match self {
            Self::V1(manifest) => manifest.roles.clone(),
        }
    }

    /// Derive a manifest from a list of InstalledCells, filling in some values
    /// with defaults.
    pub fn from_legacy(cells: impl Iterator<Item = InstalledCell>) -> Self {
        let roles = cells
            .map(|InstalledCell { role_name, .. }| {
                let path = PathBuf::from(role_name.clone());
                AppRoleManifest {
                    name: role_name,
                    provisioning: None,
                    dna: AppRoleDnaManifest {
                        location: Some(mr_bundle::Location::Bundled(path)),
                        modifiers: Default::default(),
                        installed_hash: None,
                        clone_limit: 256,
                    },
                }
            })
            .collect();

        AppManifestCurrent {
            name: "[autogenerated manifest]".into(),
            description: Some("Generated by `fn new_legacy`".into()),
            roles,
            membrane_proofs_deferred: false,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {

    use mr_bundle::Manifest;

    use crate::app::app_manifest::{AppManifest, AppManifestV1Builder, AppRoleManifest};

    #[test]
    /// Replicate this test for any new version of the manifest that gets created
    fn app_manifest_v1_helper_functions() {
        let app_name = String::from("sample-app");

        let role_name = String::from("sample-dna");
        let role_manifest = AppRoleManifest::sample(role_name);

        let sample_app_manifest_v1 = AppManifestV1Builder::default()
            .name(app_name.clone())
            .description(Some(String::from("Some description")))
            .roles(vec![role_manifest.clone()])
            .build()
            .unwrap();
        let sample_app_manifest = AppManifest::V1(sample_app_manifest_v1.clone());

        assert_eq!(app_name, sample_app_manifest.app_name());
        assert_eq!(vec![role_manifest], sample_app_manifest.app_roles());
        assert_eq!(
            vec![sample_app_manifest_v1
                .roles
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
