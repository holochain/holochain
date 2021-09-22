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

    /// Get the bundle location of the Web UI zip included in the manifest
    pub fn web_ui_location(&self) -> Location {
        match self {
            Self::V1(WebAppManifestV1 { ui, .. }) => ui.location.clone(),
        }
    }

    /// Get the location of the app bundle included in the manifest
    pub fn happ_bundle_location(&self) -> Location {
        match self {
            Self::V1(WebAppManifestV1 { happ_manifest, .. }) => happ_manifest.location.clone(),
        }
    }
}

#[cfg(test)]
pub mod tests {

    use crate::web_app::{
        web_app_manifest::WebAppManifestV1, AppManifestLocation,
        WebAppManifest, WebUI,
    };
    use mr_bundle::{Location, Manifest};

    #[test]
    /// Replicate this test for any new version of the manifest that gets created
    fn web_app_manifest_v1_helper_functions() {
        let ui_location = Location::Bundled("./path/to/my/ui.zip".into());
        let happ_location = Location::Bundled("./path/to/my/happ-bundle.happ".into());
        let app_name = String::from("sample-web-happ");
        let web_app_manifest = WebAppManifest::V1(WebAppManifestV1 {
            name: app_name.clone(),
            ui: WebUI {
                location: ui_location.clone(),
            },
            happ_manifest: AppManifestLocation {
                location: happ_location.clone(),
            },
        });

        assert_eq!(WebAppManifest::current(app_name.clone()), web_app_manifest);

        assert_eq!(
            vec![ui_location.clone(), happ_location.clone()],
            web_app_manifest.locations()
        );
        assert_eq!(app_name, web_app_manifest.app_name());
        assert_eq!(ui_location, web_app_manifest.web_ui_location());
        assert_eq!(happ_location, web_app_manifest.happ_bundle_location());
    }
}
