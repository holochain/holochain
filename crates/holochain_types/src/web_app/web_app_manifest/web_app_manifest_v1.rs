//! WebApp Manifest format, version 1.
//!
//! NB: After stabilization, *do not modify this file*! Create a new version of

//! the spec and leave this one alone to maintain backwards compatibility.

/// Version 1 of the App manifest schema
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct WebAppManifestV1 {
    /// Name of the App. This may be used as the installed_app_id.
    pub name: String,

    /// Web UI used for this app, packaged in a .zip file
    pub ui: WebUI,

    /// The Cell manifests that make up this app.
    pub happ_manifest: AppManifestLocation,
}

/// Web UI .zip file that should be associated with the happ
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct WebUI {
    /// Where to find this UI.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    pub location: mr_bundle::Location,
}

/// Location of the happ bundle to bind with the Web UI
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AppManifestLocation {
    /// Where to find the happ for this web-happ.
    ///
    /// Note that since this is flattened,
    /// there is no actual "location" key in the manifest.
    #[serde(flatten)]
    pub location: mr_bundle::Location,
}
