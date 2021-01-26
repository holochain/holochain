use std::path::PathBuf;

/// Where to find a file.
///
/// This representation, with named fields, is chosen so that in the yaml config
/// either "path", "url", or "bundled" can be specified due to this field
/// being flattened.
#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// #[serde(from = "LocationSerialized")]
#[serde(rename_all = "lowercase")]
// #[serde(into = "LocationSerialized")]
#[allow(missing_docs)]
pub enum Location {
    /// Expect file to be part of this bundle
    Bundled(PathBuf),

    /// Get file from local filesystem (not bundled)
    Path(PathBuf),

    /// Get file from URL
    Url(String),
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
//             Location::Local(path) => Self::Local { path },
//             Location::Remote(url) => Self::Remote { url },
//         }
//     }
// }

// impl From<LocationSerialized> for Location {
//     fn from(loc: LocationSerialized) -> Self {
//         match loc {
//             LocationSerialized::Bundled { bundled } => Self::Bundled(bundled),
//             LocationSerialized::Local { path } => Self::Local(path),
//             LocationSerialized::Remote { url } => Self::Remote(url),
//         }
//     }
// }
