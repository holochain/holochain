use serde::{Deserialize, Serialize};

mod dpki_config;
mod logger_config;
mod network_config;
mod passphrase_service_config;
mod signal_config;
use super::{
    error::{ConductorError, ConductorResult},
    paths::{ConfigFilePath, EnvironmentRootPath},
};
use dpki_config::DpkiConfig;
use logger_config::LoggerConfig;
use network_config::NetworkConfig;
use passphrase_service_config::PassphraseServiceConfig;
use signal_config::SignalConfig;

#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct ConductorConfig {
    /// The path to the LMDB environment for this conductor.
    /// If omitted, chooses a default path.
    pub environment_path: EnvironmentRootPath,

    /// Configures how logging should behave. Optional.
    pub logger: LoggerConfig,

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

    /// Which signals to emitproject_root
    pub signals: SignalConfig,

    /// Configure how the conductor should prompt the user for the passphrase to lock/unlock keystores.
    /// The conductor is independent of the specialized implementation of the trait
    /// PassphraseService. It just needs something to provide a passphrase when needed.
    /// This config setting selects one of the available services (i.e. CLI prompt, IPC, mock)
    pub passphrase_service: PassphraseServiceConfig,
}

impl ConductorConfig {
    pub fn load_toml(path: ConfigFilePath) -> ConductorResult<ConductorConfig> {
        // let path_buf: &Path = path.into();
        let content_toml = std::fs::read_to_string(path.as_ref()).map_err(|err| match err {
            e @ std::io::Error { .. } if e.kind() == std::io::ErrorKind::NotFound => {
                ConductorError::ConfigMissing(path.into())
            }
            _ => err.into(),
        })?;
        Ok(toml::from_str(&content_toml)?)
    }
}
