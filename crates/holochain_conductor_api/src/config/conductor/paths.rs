pub use holochain_keystore::paths::*;
use std::path::PathBuf;

/// Subdirectory of the data directory where the conductor stores its
/// databases.
pub const DATABASES_DIRECTORY: &str = "databases";

/// Subdirectory of the data directory where the conductor stores its
/// compiled wasm.
pub const WASM_DIRECTORY: &str = "wasm";

/// Newtype to make sure we never accidentaly use or not use the config path.
/// Intentionally has no default value.
#[derive(
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    Clone,
)]
pub struct ConfigPath(PathBuf);

/// Newtype to make sure we never accidentaly use or not use the data path.
/// Intentionally has no default value.
#[derive(
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    Clone,
)]
pub struct DataPath(PathBuf);

impl From<DataPath> for KeystorePath {
    fn from(data_path: DataPath) -> Self {
        Self::from(data_path.0.join(KEYSTORE_DIRECTORY))
    }
}
