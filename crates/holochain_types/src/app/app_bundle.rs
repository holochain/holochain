use super::AppManifest;
use crate::prelude::*;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From)]
pub struct AppBundle(mr_bundle::Bundle<AppManifest>);

/// Alias for mr_bundle Bundler errors
pub type AppBundleError = mr_bundle::error::BundleError;

impl AppBundle {
    // pub fn dnas(&self) -> Vec<DnaFile> {
    //     todo!()
    // }
}
