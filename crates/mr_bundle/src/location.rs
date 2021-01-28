use crate::error::MrBundleResult;
use bytes::Bytes;
use std::path::{Path, PathBuf};

/// Where to find a file.
///
/// This representation, with named fields, is chosen so that in the yaml config
/// either "path", "url", or "bundled" can be specified due to this field
/// being flattened.
#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// #[serde(from = "LocationSerialized", into = "LocationSerialized")]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Location {
    /// Expect file to be part of this bundle
    Bundled(PathBuf),

    /// Get file from local filesystem (not bundled)
    Path(PathBuf),

    /// Get file from URL
    Url(String),
}

pub(crate) async fn resolve_local(path: &Path) -> MrBundleResult<Bytes> {
    Ok(std::fs::read(path)?.into())
}

pub(crate) async fn resolve_remote(url: &str) -> MrBundleResult<Bytes> {
    Ok(reqwest::get(url).await?.bytes().await?)
}
