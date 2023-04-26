//! Helpers for creating, reading and writing [`ConductorConfig`]s.

use std::path::PathBuf;

use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;

/// Name of the file that conductor config is written to.
pub const CONDUCTOR_CONFIG: &str = "conductor-config.yaml";

/// Create a new default [`ConductorConfig`] with environment path,
/// keystore, and database all in the same directory.
pub fn create_config(environment_path: PathBuf, con_url: Option<url2::Url2>) -> ConductorConfig {
    let mut conductor_config = ConductorConfig {
        environment_path: environment_path.clone().into(),
        ..Default::default()
    };
    let mut keystore_path = environment_path.clone();
    keystore_path.push("keystore");
    match con_url {
        Some(url) => {
            conductor_config.keystore = KeystoreConfig::LairServer {
                connection_url: url,
            };
        }
        None => {
            let mut lair_root = environment_path;
            // Keep the path short so that when it's used in CI the path doesn't get too long to be used as a domain socket
            lair_root.push("ks");
            conductor_config.keystore = KeystoreConfig::LairServerInProc {
                lair_root: Some(lair_root),
            };
        }
    }
    conductor_config
}

/// Write [`ConductorConfig`] to [`CONDUCTOR_CONFIG`].
pub fn write_config(mut path: PathBuf, config: &ConductorConfig) -> PathBuf {
    path.push(CONDUCTOR_CONFIG);
    std::fs::write(path.clone(), serde_yaml::to_string(&config).unwrap()).unwrap();
    path
}

/// Read the [`ConductorConfig`] from the file [`CONDUCTOR_CONFIG`] in the provided path.
pub fn read_config(mut path: PathBuf) -> anyhow::Result<Option<ConductorConfig>> {
    path.push(CONDUCTOR_CONFIG);

    match std::fs::read_to_string(path) {
        Ok(yaml) => Ok(Some(serde_yaml::from_str(&yaml)?)),
        Err(_) => Ok(None),
    }
}
