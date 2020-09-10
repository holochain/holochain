#![deny(missing_docs)]
//! This module is used to configure the conductor

use serde::{Deserialize, Serialize};

mod admin_interface_config;
mod dpki_config;
mod network_config;
mod passphrase_service_config;
//mod logger_config;
//mod signal_config;
use super::{
    error::{ConductorError, ConductorResult},
    paths::EnvironmentRootPath,
};

pub use crate::conductor::interface::InterfaceDriver;
pub use admin_interface_config::AdminInterfaceConfig;
pub use dpki_config::DpkiConfig;
//pub use logger_config::LoggerConfig;
pub use network_config::NetworkConfig;
pub use passphrase_service_config::PassphraseServiceConfig;
//pub use signal_config::SignalConfig;
use std::path::Path;

// TODO change types from "stringly typed" to Url2
/// All the config information for the conductor
#[derive(Deserialize, Serialize, Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
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
    pub passphrase_service: Option<PassphraseServiceConfig>,

    /// Setup admin interfaces to control this conductor through a websocket connection
    pub admin_interfaces: Option<Vec<AdminInterfaceConfig>>,

    /// Enabling this will use a test keystore instead of lair.
    /// This generates publicly accessible private keys.
    /// DO NOT USE THIS IN PRODUCTION!
    #[serde(default)]
    pub use_dangerous_test_keystore: bool,
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
fn config_from_toml<'a, T>(toml: &'a str) -> ConductorResult<T>
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
    use matches::assert_matches;
    use std::path::{Path, PathBuf};
    use url::Url;

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
        assert_matches!(result, Err(ConductorError::DeserializationError(_)));
    }

    #[test]
    fn test_config_complete_minimal_config() {
        let toml = r#"
    environment_path = "/path/to/env"

    [passphrase_service]
    type = "cmd"
    "#;
        let result: ConductorConfig = config_from_toml(toml).unwrap();
        assert_eq!(
            result,
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                network: None,
                signing_service_uri: None,
                encryption_service_uri: None,
                decryption_service_uri: None,
                dpki: None,
                passphrase_service: Some(PassphraseServiceConfig::Cmd),
                admin_interfaces: None,
                use_dangerous_test_keystore: false,
            }
        );
    }

    #[test]
    fn test_config_complete_config() {
        let toml = r#"
    environment_path = "/path/to/env"
    use_dangerous_test_keystore = true

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

    [[admin_interfaces]]
    driver.type = "websocket"
    driver.port = 1234

    "#;
        let result: ConductorResult<ConductorConfig> = config_from_toml(toml);
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                network: Some(NetworkConfig::Sim2h {
                    url: Url::parse("ws://localhost:9000/").unwrap()
                }),
                signing_service_uri: None,
                encryption_service_uri: None,
                decryption_service_uri: None,
                dpki: Some(DpkiConfig {
                    instance_id: "some_id".into(),
                    init_params: "some_params".into()
                }),
                passphrase_service: Some(PassphraseServiceConfig::Cmd),
                admin_interfaces: Some(vec![AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket { port: 1234 }
                }]),
                use_dangerous_test_keystore: true,
            }
        );
    }
}
