use mr_bundle::error::MrBundleResult;

use super::WebAppManifest;
use crate::prelude::*;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct WebAppBundle(mr_bundle::Bundle<WebAppManifest>);

impl WebAppBundle {
    /// Construct from raw bytes
    pub fn decode(bytes: &[u8]) -> MrBundleResult<Self> {
        mr_bundle::Bundle::decode(bytes).map(|b| WebAppBundle(b))
    }
}
