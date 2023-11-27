//! Helpers for creating, reading and writing [`ConductorConfig`]s.

use std::path::PathBuf;

use holochain_conductor_api::conductor::paths::DataPath;
use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;

/// Name of the file that conductor config is written to.
pub const CONDUCTOR_CONFIG: &str = "conductor-config.yaml";

/// Create a new default [`ConductorConfig`] with data_root_path path,
/// keystore, and database all in the same directory.
pub fn create_config(data_root_path: DataPath, con_url: Option<url2::Url2>) -> ConductorConfig {
    let mut conductor_config = ConductorConfig {
        data_root_path: Some(data_root_path.clone()),
        ..Default::default()
    };
    match con_url {
        Some(url) => {
            conductor_config.keystore = KeystoreConfig::LairServer {
                connection_url: url,
            };
        }
        None => {
            conductor_config.keystore = KeystoreConfig::LairServerInProc {
                lair_root: Some(data_root_path.into()),
            };
        }
    }
    conductor_config
}

/// Write [`ConductorConfig`] to [`CONDUCTOR_CONFIG`].
/// This treats a data path as a config path, rather than respecting a
/// separation of concerns there.
pub fn write_config(data_root_path: DataPath, config: &ConductorConfig) -> PathBuf {
    let path = data_root_path.as_ref().join(CONDUCTOR_CONFIG);
    std::fs::write(path.clone(), serde_yaml::to_string(&config).unwrap()).unwrap();
    path
}

/// Read the [`ConductorConfig`] from the file [`CONDUCTOR_CONFIG`] in the provided path.
/// This treats a data path as a config path, rather than respecting a
/// separation of concerns there.
pub fn read_config(data_root_path: DataPath) -> anyhow::Result<Option<ConductorConfig>> {
    let path = data_root_path.as_ref().join(CONDUCTOR_CONFIG);

    match std::fs::read_to_string(path) {
        Ok(yaml) => Ok(Some(serde_yaml::from_str(&yaml)?)),
        Err(_) => Ok(None),
    }
}
