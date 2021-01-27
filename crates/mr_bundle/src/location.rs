use std::path::{Path, PathBuf};

use bytes::Bytes;

use crate::error::MrBundleResult;

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

impl Location {
    pub async fn resolve(&self) -> MrBundleResult<Bytes> {
        match self {
            Self::Bundled(path) => todo!(),
            Self::Path(path) => resolve_local(path).await,
            Self::Url(url) => resolve_remote(url).await,
        }
    }
}

async fn resolve_local(path: &Path) -> MrBundleResult<Bytes> {
    Ok(std::fs::read(path)?.into())
}

async fn resolve_remote(url: &str) -> MrBundleResult<Bytes> {
    Ok(reqwest::get(url).await?.bytes().await?)
}

// #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// #[serde(rename_all = "snake_case")]
// #[serde(untagged)]
// #[allow(missing_docs)]
// pub enum LocationSerialized {
//     /// Expect file to be part of this bundle
//     Bundled { bundled: PathBuf },

//     /// Get file from local filesystem (not bundled)
//     Local { path: PathBuf },

//     /// Get file from URL
//     Remote { url: String },
// }

// impl From<Location> for LocationSerialized {
//     fn from(loc: Location) -> Self {
//         match loc {
//             Location::Bundled(bundled) => Self::Bundled { bundled },
//             Location::Path(path) => Self::Local { path },
//             Location::Url(url) => Self::Remote { url },
//         }
//     }
// }

// impl From<LocationSerialized> for Location {
//     fn from(loc: LocationSerialized) -> Self {
//         match loc {
//             LocationSerialized::Bundled { bundled } => Self::Bundled(bundled),
//             LocationSerialized::Local { path } => Self::Path(path),
//             LocationSerialized::Remote { url } => Self::Url(url),
//         }
//     }
// }
