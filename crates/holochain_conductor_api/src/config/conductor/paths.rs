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
    pub fn is_also_data_root_path(&self) -> DataRootPath {
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
pub struct DataRootPath(PathBuf);

impl TryFrom<DataRootPath> for KeystorePath {
    type Error = std::io::Error;
    fn try_from(data_root_path: DataRootPath) -> Result<Self, Self::Error> {
        let path = data_root_path.0.join(KEYSTORE_DIRECTORY);
        if let Ok(false) = path.try_exists() {
            std::fs::create_dir_all(path.clone())?;
        }
        Ok(Self::from(path))
    }
}

impl From<DataRootPath> for PathBuf {
    fn from(value: DataRootPath) -> Self {
        value.0
    }
}

/// Newtype to make sure we never accidentaly use or not use the databases path.
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
pub struct DatabasesRootPath(PathBuf);

impl TryFrom<DataRootPath> for DatabasesRootPath {
    type Error = std::io::Error;
    fn try_from(data_path: DataRootPath) -> Result<Self, Self::Error> {
        let path = data_path.0.join(DATABASES_DIRECTORY);
        if let Ok(false) = path.try_exists() {
            std::fs::create_dir_all(path.clone())?;
        }
        Ok(Self::from(path))
    }
}

/// Newtype to make sure we never accidentaly use or not use the wasm path.
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
pub struct WasmRootPath(PathBuf);

impl TryFrom<DataRootPath> for WasmRootPath {
    type Error = std::io::Error;
    fn try_from(data_path: DataRootPath) -> Result<Self, Self::Error> {
        let path = data_path.0.join(WASM_DIRECTORY);
        if let Ok(false) = path.try_exists() {
            std::fs::create_dir_all(path.clone())?;
        }
        Ok(Self::from(path))
    }
}
