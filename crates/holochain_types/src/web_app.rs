//! Web App manifest describing how to bind a Web UI and a happ bundle together
//!
//! This will not be used inside Holochain as the bundle to install, but rather as a
//! unique package that both Holo and the launcher know how to install, in slightly
//! different ways.
//!
//! Eg: when the launcher installs a web-happ bundle, it will extract the WebUI and
//! install it in the file system. Also, it will extract the happ bundle and call
//! `InstallApp` with it.

mod web_app_bundle;
mod web_app_manifest;

pub use web_app_bundle::*;
pub use web_app_manifest::*;
