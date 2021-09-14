#![warn(missing_docs)]

//! Defines the hApp Manifest YAML format, including validation.

use mr_bundle::{Location, Manifest};
use std::path::PathBuf;

mod current;
pub(crate) mod web_app_manifest_v1;

pub use current::*;

use web_app_manifest_v1::WebAppManifestV1;

/// Container struct which uses the `manifest_version` field to determine
/// which manifest version to deserialize to.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum WebAppManifest {
    #[serde(rename = "1")]
    V1(WebAppManifestV1),
}

impl Manifest for WebAppManifest {
    fn locations(&self) -> Vec<Location> {
        match self {
            WebAppManifest::V1(m) => vec![m.ui.location.clone(), m.happ_manifest.location.clone()],
        }
    }

    fn path() -> PathBuf {
        "web-happ.yaml".into()
    }

    fn bundle_extension() -> &'static str {
        "webhapp"
    }
}

impl WebAppManifest {
    /// Get the default manifest for the current version
    pub fn current(name: String) -> Self {
        WebAppManifest::V1(WebAppManifestV1 {
            name,
            ui: WebUI {
                location: Location::Bundled("./path/to/my/ui.zip".into()),
            },
            happ_manifest: AppManifestLocation {
                location: Location::Bundled("./path/to/my/happ-bundle.happ".into()),
            },
        })
    }

    /// Get the supplied name of the app
    pub fn app_name(&self) -> &str {
        match self {
            Self::V1(WebAppManifestV1 { name, .. }) => name,
        }
    }
}
