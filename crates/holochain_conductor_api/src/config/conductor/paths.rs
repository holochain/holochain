//! Defines default paths for various resources

use derive_more::AsRef;
use derive_more::Display;
use derive_more::From;
use derive_more::FromStr;
use derive_more::Into;
use std::path::Path;
use std::path::PathBuf;

const QUALIFIER: &str = "org";
const ORGANIZATION: &str = "holochain";
const APPLICATION: &str = "holochain";
const KEYS_DIRECTORY: &str = "keys";
const DATABASES_DIRECTORY: &str = "databases";
const CONFIG_FILENAME: &str = "conductor-config.yml";

/// Newtype for the database path. Has a Default.
#[derive(
    Clone,
    From,
    Into,
    Debug,
    PartialEq,
    AsRef,
    FromStr,
    Display,
    serde::Serialize,
    serde::Deserialize,
)]
#[display(fmt = "{}", "_0.display()")]
pub struct EnvironmentRootPath(PathBuf);
impl Default for EnvironmentRootPath {
    fn default() -> Self {
        Self(data_root().join(PathBuf::from(DATABASES_DIRECTORY)))
    }
}

impl<'a> From<&'a Path> for EnvironmentRootPath {
    fn from(p: &'a Path) -> Self {
        p.to_owned().into()
    }
}

/// Returns the project root builder for holochain directories.
fn project_root() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
}

/// Returns the path to the root config directory for all of Holochain.
/// If we can get a user directory it will be an XDG compliant path
/// like "/home/peter/.config/holochain".
/// If it can't get a user directory it will default to "/etc/holochain".
pub fn config_root() -> PathBuf {
    project_root()
        .map(|dirs| dirs.config_dir().to_owned())
        .unwrap_or_else(|| PathBuf::from("/etc").join(APPLICATION))
}

/// Returns the path to the root data directory for all of Holochain.
/// If we can get a user directory it will be an XDG compliant path
/// like "/home/peter/.local/share/holochain".
/// If it can't get a user directory it will default to "/etc/holochain".
pub fn data_root() -> PathBuf {
    project_root()
        .map(|dirs| dirs.data_dir().to_owned())
        .unwrap_or_else(|| PathBuf::from("/etc").join(APPLICATION))
}

/// Returns the path to where agent keys are stored and looked for by default.
/// Something like "~/.config/holochain/keys".
pub fn keys_directory() -> PathBuf {
    config_root().join(KEYS_DIRECTORY)
}

/// Newtype for the Conductor Config file path. Has a Default.
#[derive(
    Clone,
    From,
    Into,
    Debug,
    PartialEq,
    AsRef,
    FromStr,
    Display,
    serde::Serialize,
    serde::Deserialize,
)]
#[display(fmt = "{}", "_0.display()")]
pub struct ConfigFilePath(PathBuf);
impl Default for ConfigFilePath {
    fn default() -> Self {
        Self(config_root().join(PathBuf::from(CONFIG_FILENAME)))
    }
}
