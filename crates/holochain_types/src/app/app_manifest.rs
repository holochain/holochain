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
//! is mainly defined by a collection of "slots",
//! each of which may be populated with Cells (instances of DNA) either during
//! app installation or during runtime. Aside from the slot definitions, an
//! app also has a `name`, which is used as the `installed_app_id` and must be
//! globally unique, as well as a `description`, which is intended for humans only.
//!
//! Each Slot definition specifies what kind of Cell can occupy it.
//! You can think of a Slot as a declaration of some piece of functionality
//! that an app needs in order to function, which will be provided by some Cell
//! in a flexible manner depending on the state of the conductor at the time of
//! installation.
//!
//! Each Slot definition is made up of:
//! - a SlotId, which only needs to be unique within this App
//! - a provisioning strategy, [`CellProvisioning`], which describes if and how a Cell
//!   should be created freshly for this app, or whether an existing Cell should
//!   occupy this slot
//! - a DNA descriptor, [`AppSlotDnaManifest`], which describes where to find the DNA,
//!   the acceptable range of versions, and the cloning limitations.

use mr_bundle::{Location, Manifest};
use std::path::PathBuf;

pub(crate) mod app_manifest_v1;
pub mod app_manifest_validated;
mod current;
mod error;

pub use app_manifest_v1::{AppSlotDnaManifest, CellProvisioning};
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
}
