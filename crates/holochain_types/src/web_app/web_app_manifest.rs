#![warn(missing_docs)]

//! Defines the hApp Manifest YAML format, including validation.

use mr_bundle::{resource_id_for_path, Manifest, ResourceIdentifier};
use schemars::JsonSchema;
use std::collections::HashMap;

mod current;
pub(crate) mod web_app_manifest_v1;

pub use current::*;

fn resource_id_for_ui(file: &str) -> ResourceIdentifier {
    resource_id_for_path(file).unwrap_or("ui.zip".to_string())
}

fn resource_id_for_happ(file: &str) -> ResourceIdentifier {
    resource_id_for_path(file).unwrap_or("happ-bundle.happ".to_string())
}

/// Container struct which uses the `manifest_version` field to determine
/// which manifest version to deserialize to.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema, derive_more::From,
)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
pub enum WebAppManifest {
    #[serde(rename = "1")]
    V1(WebAppManifestV1),
}

impl Manifest for WebAppManifest {
    fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
        match self {
            WebAppManifest::V1(m) => {
                let mut out = HashMap::new();

                let ui_id = resource_id_for_ui(&m.ui.path);
                out.insert(ui_id.clone(), m.ui.path.clone());
                m.ui.path = ui_id;

                let happ_id = resource_id_for_happ(&m.happ.path);
                out.insert(happ_id.clone(), m.happ.path.clone());
                m.happ.path = happ_id;

                out
            }
        }
    }

    fn resource_ids(&self) -> Vec<ResourceIdentifier> {
        match self {
            WebAppManifest::V1(m) => vec![
                resource_id_for_ui(&m.ui.path),
                resource_id_for_happ(&m.happ.path),
            ],
        }
    }

    fn file_name() -> &'static str {
        "web-happ.yaml"
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
                path: "./path/to/my/ui.zip".to_string(),
            },
            happ: AppManifestLocation {
                path: "./path/to/my/happ-bundle.happ".to_string(),
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
    pub fn web_ui_location(&self) -> ResourceIdentifier {
        match self {
            Self::V1(WebAppManifestV1 { ui, .. }) => resource_id_for_ui(&ui.path),
        }
    }

    /// Get the location of the app bundle included in the manifest
    pub fn happ_bundle_location(&self) -> ResourceIdentifier {
        match self {
            Self::V1(WebAppManifestV1 { happ, .. }) => resource_id_for_happ(&happ.path),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::web_app::{
        web_app_manifest::WebAppManifestV1, AppManifestLocation, WebAppManifest, WebUI,
    };
    use mr_bundle::Manifest;

    #[test]
    /// Replicate this test for any new version of the manifest that gets created
    fn web_app_manifest_v1_helper_functions() {
        let ui_location = "./path/to/my/ui.zip".to_string();
        let happ_location = "./path/to/my/happ-bundle.happ".to_string();
        let app_name = String::from("sample-web-happ");
        let web_app_manifest = WebAppManifest::V1(WebAppManifestV1 {
            name: app_name.clone(),
            ui: WebUI {
                path: ui_location.clone(),
            },
            happ: AppManifestLocation {
                path: happ_location.clone(),
            },
        });

        assert_eq!(WebAppManifest::current(app_name.clone()), web_app_manifest);

        let ui_id = super::resource_id_for_ui(&ui_location);
        let happ_id = super::resource_id_for_happ(&happ_location);
        assert_eq!(
            vec![ui_id.clone(), happ_id.clone()],
            web_app_manifest.resource_ids()
        );
        assert_eq!(app_name, web_app_manifest.app_name());
        assert_eq!(ui_id, web_app_manifest.web_ui_location());
        assert_eq!(happ_id, web_app_manifest.happ_bundle_location());
    }
}
