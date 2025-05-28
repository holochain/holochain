use crate::app::AppBundle;
use crate::web_app::WebAppManifest;
use bytes::Buf;
use mr_bundle::error::MrBundleError;
use mr_bundle::error::MrBundleResult;
use mr_bundle::Bundle;
use serde_derive::{Deserialize, Serialize};
use std::io::Read;

/// A bundle of an AppBundle and a Web UI bound with it
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct WebAppBundle(Bundle<WebAppManifest>);

impl WebAppBundle {
    /// Construct from raw bytes
    pub fn unpack(bytes: impl Read) -> MrBundleResult<Self> {
        Bundle::unpack(bytes).map(WebAppBundle)
    }

    /// Returns the bytes of the zip file containing the Web UI contained inside this WebAppBundle
    pub async fn web_ui_zip_bytes(&self) -> MrBundleResult<bytes::Bytes> {
        let manifest = self.0.manifest();

        let ui_location = manifest.web_ui_location();
        self.0
            .get_resource(&ui_location)
            .ok_or_else(|| MrBundleError::MissingResources(vec![ui_location]))
            .cloned()
            .map(Into::into)
    }

    /// Returns the hApp bundle contained inside this WebAppBundle
    pub async fn happ_bundle(&self) -> MrBundleResult<AppBundle> {
        let manifest = self.0.manifest();

        let happ_location = manifest.happ_bundle_location();
        let bytes: bytes::Bytes = self
            .0
            .get_resource(&happ_location)
            .ok_or_else(|| MrBundleError::MissingResources(vec![happ_location]))
            .cloned()
            .map(Into::into)?;

        let bundle = AppBundle::from(Bundle::unpack(bytes.reader())?);

        Ok(bundle)
    }
}
