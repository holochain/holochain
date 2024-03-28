//! Helpers for creating, reading and writing [`ConductorConfig`]s.

use holochain_conductor_api::conductor::paths::ConfigFilePath;
use holochain_conductor_api::conductor::paths::ConfigRootPath;
use holochain_conductor_api::config::conductor::ConductorConfig;
use holochain_conductor_api::config::conductor::KeystoreConfig;

/// Create a new default [`ConductorConfig`] with data_root_path path,
/// keystore, and database all in the same directory.
pub fn create_config(
    config_root_path: ConfigRootPath,
    con_url: Option<url2::Url2>,
) -> anyhow::Result<ConductorConfig> {
    let mut conductor_config = ConductorConfig {
        data_root_path: Some(config_root_path.is_also_data_root_path()),
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
                lair_root: Some(config_root_path.is_also_data_root_path().try_into()?),
                pw_hash_strength: None,
            };
        }
    }
    Ok(conductor_config)
}

/// Write [`ConductorConfig`] to [`CONDUCTOR_CONFIG`].
pub fn write_config(config_root_path: ConfigRootPath, config: &ConductorConfig) -> ConfigFilePath {
    let config_file_path: ConfigFilePath = config_root_path.into();
    std::fs::write(
        config_file_path.as_ref(),
        serde_yaml::to_string(&config).unwrap(),
    )
    .unwrap();
    config_file_path
}

/// Read the [`ConductorConfig`] from the file [`CONDUCTOR_CONFIG`] in the provided path.
pub fn read_config(config_root_path: ConfigRootPath) -> anyhow::Result<Option<ConductorConfig>> {
    match std::fs::read_to_string(ConfigFilePath::from(config_root_path).as_ref()) {
        Ok(yaml) => Ok(Some(serde_yaml::from_str(&yaml)?)),
        Err(_) => Ok(None),
    }
}
