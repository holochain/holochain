use std::borrow::Cow;

use mr_bundle::{error::MrBundleResult, ResourceBytes};

use super::WebAppManifest;
use crate::prelude::*;
use mr_bundle::Bundle;

/// A bundle of an AppBundle and a Web UI bound with it
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct WebAppBundle(Bundle<WebAppManifest>);

impl WebAppBundle {
    /// Construct from raw bytes
    pub fn decode(bytes: &[u8]) -> MrBundleResult<Self> {
        Bundle::decode(bytes).map(WebAppBundle)
    }

    /// Returns the bytes of the zip file containing the Web UI contained inside this WebAppBundle
    pub async fn web_ui_zip_bytes(&self) -> MrBundleResult<Cow<'_, ResourceBytes>> {
        let manifest = self.0.manifest();

        self.0.resolve(&manifest.web_ui_location()).await
    }

    /// Returns the hApp bundle contained inside this WebAppBundle
    pub async fn happ_bundle(&self) -> MrBundleResult<AppBundle> {
        let manifest = self.0.manifest();

        let bytes = self.0.resolve(&manifest.happ_bundle_location()).await?;
        let bundle = AppBundle::from(Bundle::decode(&bytes)?);
        Ok(bundle)
    }
}
