pub use holochain_keystore::paths::*;
use std::path::PathBuf;

/// Subdirectory of the data directory where the conductor stores its
/// databases.
pub const DATABASES_DIRECTORY: &str = "databases";

/// Subdirectory of the data directory where the conductor stores its
/// compiled wasm.
pub const WASM_DIRECTORY: &str = "wasm";

/// Name of the file that conductor config is written to.
pub const CONDUCTOR_CONFIG: &str = "conductor-config.yaml";

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
pub struct ConfigRootPath(PathBuf);

impl ConfigRootPath {
    /// Create a new config root path from a data path.
    /// This is useful for when you want to use the same path for both.
    pub fn is_also_data_root_path(&self) -> DataPath {
        self.0.clone().into()
    }
}

/// Newtype to make sure we never accidentaly use or not use the config file
/// path.
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
pub struct ConfigFilePath(PathBuf);

impl From<ConfigRootPath> for ConfigFilePath {
    fn from(config_path: ConfigRootPath) -> Self {
        Self::from(config_path.0.join(CONDUCTOR_CONFIG))
    }
}

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
