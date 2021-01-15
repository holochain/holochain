use std::path::PathBuf;

use holochain_conductor_api::config::conductor::ConductorConfig;

pub(crate) fn create_config(environment_path: PathBuf) -> ConductorConfig {
    let mut conductor_config = ConductorConfig::default();
    conductor_config.environment_path = environment_path.clone().into();
    let mut keystore_path = environment_path;
    keystore_path.push("keystore");
    conductor_config.keystore_path = Some(keystore_path);
    conductor_config
}

pub(crate) fn write_config(mut path: PathBuf, config: &ConductorConfig) -> PathBuf {
    path.push("conductor-config.yaml");
    std::fs::write(path.clone(), serde_yaml::to_string(&config).unwrap()).unwrap();
    path
}

pub(crate) fn read_config(mut path: PathBuf) -> anyhow::Result<Option<ConductorConfig>> {
    path.push("conductor-config.yaml");

    match std::fs::read_to_string(path) {
        Ok(yaml) => Ok(Some(serde_yaml::from_str(&yaml)?)),
        Err(_) => Ok(None),
    }
}
