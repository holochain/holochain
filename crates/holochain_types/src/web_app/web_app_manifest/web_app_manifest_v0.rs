//! WebApp Manifest format, version 0.
//!
//! NB: After stabilization, *do not modify this file*! Create a new version of
//! the spec and leave this one alone to maintain backwards compatibility.

use schemars::JsonSchema;

/// Version 0 of the WebApp manifest schema
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    JsonSchema,
    derive_builder::Builder,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct WebAppManifestV0 {
    /// Name of the App. This may be used as the installed_app_id.
    pub name: String,

    /// Web UI used for this app, packaged in a .zip file
    pub ui: WebUI,

    /// The Cell manifests that make up this app.
    pub happ: AppManifestLocation,
}

/// Web UI .zip file that should be associated with the hApp.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct WebUI {
    /// Where to find this UI.
    pub path: String,
}

/// Location of the hApp bundle to bind with the Web UI.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct AppManifestLocation {
    /// Where to find the hApp for this web-happ.
    pub path: String,
}
