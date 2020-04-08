use serde::{Deserialize, Serialize};

mod dpki_config;
//mod logger_config;
mod network_config;
mod passphrase_service_config;
//mod signal_config;
use super::{
    error::{ConductorError, ConductorResult},
    paths::EnvironmentRootPath,
};
use dpki_config::DpkiConfig;
//use logger_config::LoggerConfig;
use network_config::NetworkConfig;
use passphrase_service_config::PassphraseServiceConfig;
//use signal_config::SignalConfig;
use std::path::Path;

#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct ConductorConfig {
    /// The path to the LMDB environment for this conductor.
    /// If omitted, chooses a default path.
    pub environment_path: EnvironmentRootPath,

    /// Config options for the network module. Optional.
    pub network: Option<NetworkConfig>,

    /// Optional URI for a websocket connection to an outsourced signing service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub signing_service_uri: Option<String>,

    /// Optional URI for a websocket connection to an outsourced encryption service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub encryption_service_uri: Option<String>,

    /// Optional URI for a websocket connection to an outsourced decryption service.
    /// Bootstrapping step for Holo closed-alpha.
    /// If set, all agents with holo_remote_key = true will be emulated by asking for signatures
    /// over this websocket.
    pub decryption_service_uri: Option<String>,

    /// Optional DPKI configuration if conductor is using a DPKI app to initalize and manage
    /// keys for new instances
    pub dpki: Option<DpkiConfig>,

    /// Configure how the conductor should prompt the user for the passphrase to lock/unlock keystores.
    /// The conductor is independent of the specialized implementation of the trait
    /// PassphraseService. It just needs something to provide a passphrase when needed.
    /// This config setting selects one of the available services (i.e. CLI prompt, IPC, mock)
    pub passphrase_service: PassphraseServiceConfig,
    //
    //
    // /// Which signals to emit
    // TODO: it's an open question whether signal config is stateful or not, i.e. whether it belongs here.
    // pub signals: SignalConfig,
    //
    // /// Configures how logging should behave. Optional.
    // TODO: it's an open question whether we want to keep any of the legacy LoggerConfig
    // pub logger: LoggerConfig,
}

/// helper fnction function to load a `Config` from a toml string.
pub fn config_from_toml<'a, T>(toml: &'a str) -> ConductorResult<T>
where
    T: Deserialize<'a>,
{
    toml::from_str::<T>(toml).map_err(ConductorError::DeserializationError)
}

impl ConductorConfig {
    /// create a ConductorConfig struct from a toml file path
    pub fn load_toml(path: &Path) -> ConductorResult<ConductorConfig> {
        let config_toml = std::fs::read_to_string(path).map_err(|err| match err {
            e @ std::io::Error { .. } if e.kind() == std::io::ErrorKind::NotFound => {
                ConductorError::ConfigMissing(path.into())
            }
            _ => err.into(),
        })?;
        config_from_toml(&config_toml)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_config_load_toml() {
        let bad_path = Path::new("fake");
        let result = ConductorConfig::load_toml(bad_path);
        assert_eq!(
            "Err(ConfigMissing(\"fake\"))".to_string(),
            format!("{:?}", result)
        );

        // successful load test in conductor/interactive
    }

    #[test]
    fn test_config_bad_toml() {
        let result: ConductorResult<ConductorConfig> = config_from_toml("this isn't toml");
        assert_eq!("Err(DeserializationError(Error { inner: ErrorInner { kind: Wanted { expected: \"an equals\", found: \"an identifier\" }, line: Some(0), col: 5, at: Some(5), message: \"\", key: [] } }))".to_string(), format!("{:?}", result));
    }

    #[test]
    fn test_config_complete_minimal_config() {
        let toml = r#"
    environment_path = "/path/to/env"

    [passphrase_service]
    type = "cmd"
    "#;
        let result: ConductorResult<ConductorConfig> = config_from_toml(toml);
        assert_eq!("Ok(ConductorConfig { environment_path: EnvironmentRootPath(\"/path/to/env\"), network: None, signing_service_uri: None, encryption_service_uri: None, decryption_service_uri: None, dpki: None, passphrase_service: Cmd })", format!("{:?}", result));
    }

    #[test]
    fn test_config_complete_config() {
        let toml = r#"
    environment_path = "/path/to/env"

    [passphrase_service]
    type = "cmd"

    [network]
    type = "sim2h"
    url = "ws://localhost:9000"

    encryption_service_uri = "ws://localhost:9001"
    decryption_service_uri = "ws://localhost:9002"
    signing_service_uri = "ws://localhost:9003"

    [dpki]
    instance_id = "some_id"
    init_params = "some_params"

    "#;
        let result: ConductorResult<ConductorConfig> = config_from_toml(toml);
        assert_eq!("Ok(ConductorConfig { environment_path: EnvironmentRootPath(\"/path/to/env\"), network: Some(Sim2h { url: \"ws://localhost:9000/\" }), signing_service_uri: None, encryption_service_uri: None, decryption_service_uri: None, dpki: Some(DpkiConfig { instance_id: \"some_id\", init_params: \"some_params\" }), passphrase_service: Cmd })", format!("{:?}", result));
    }
}
