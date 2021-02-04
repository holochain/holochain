#![deny(missing_docs)]
//! This module is used to configure the conductor

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

mod admin_interface_config;
mod dpki_config;
#[allow(missing_docs)]
mod error;
mod passphrase_service_config;
pub mod paths;
//mod logger_config;
//mod signal_config;
pub use paths::EnvironmentRootPath;

pub use super::*;
pub use dpki_config::DpkiConfig;
//pub use logger_config::LoggerConfig;
pub use error::*;
pub use passphrase_service_config::PassphraseServiceConfig;
//pub use signal_config::SignalConfig;
use std::path::Path;
use std::path::PathBuf;

// TODO change types from "stringly typed" to Url2
/// All the config information for the conductor
#[derive(Clone, Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct ConductorConfig {
    /// The path to the LMDB environment for this conductor.
    /// If omitted, chooses a default path.
    pub environment_path: EnvironmentRootPath,

    /// Enabling this will use a test keystore instead of lair.
    /// This generates publicly accessible private keys.
    /// DO NOT USE THIS IN PRODUCTION!
    #[serde(default)]
    pub use_dangerous_test_keystore: bool,

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

    /// Optional path for keystore directory.  If not specified will use the default provided
    /// by the [ConfigBuilder]()https://docs.rs/lair_keystore_api/0.0.1-alpha.4/lair_keystore_api/struct.ConfigBuilder.html)
    pub keystore_path: Option<PathBuf>,

    /// Configure how the conductor should prompt the user for the passphrase to lock/unlock keystores.
    /// The conductor is independent of the specialized implementation of the trait
    /// PassphraseService. It just needs something to provide a passphrase when needed.
    /// This config setting selects one of the available services (i.e. CLI prompt, IPC, FromConfig)
    pub passphrase_service: Option<PassphraseServiceConfig>,

    /// Setup admin interfaces to control this conductor through a websocket connection
    pub admin_interfaces: Option<Vec<AdminInterfaceConfig>>,

    /// Config options for the network module. Optional.
    pub network: Option<holochain_p2p::kitsune_p2p::KitsuneP2pConfig>,
    //
    //
    // /// Which signals to emit
    // TODO: it's an open question whether signal config is stateful or not, i.e. whether it belongs here.
    // pub signals: SignalConfig,
}

/// helper fnction function to load a `Config` from a yaml string.
fn config_from_yaml<T>(yaml: &str) -> ConductorConfigResult<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(yaml).map_err(ConductorConfigError::SerializationError)
}

impl ConductorConfig {
    /// create a ConductorConfig struct from a yaml file path
    pub fn load_yaml(path: &Path) -> ConductorConfigResult<ConductorConfig> {
        let config_yaml = std::fs::read_to_string(path).map_err(|err| match err {
            e @ std::io::Error { .. } if e.kind() == std::io::ErrorKind::NotFound => {
                ConductorConfigError::ConfigMissing(path.into())
            }
            _ => err.into(),
        })?;
        config_from_yaml(&config_yaml)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use matches::assert_matches;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn test_config_load_yaml() {
        let bad_path = Path::new("fake");
        let result = ConductorConfig::load_yaml(bad_path);
        assert_eq!(
            "Err(ConfigMissing(\"fake\"))".to_string(),
            format!("{:?}", result)
        );

        // successful load test in conductor/interactive
    }

    #[test]
    fn test_config_bad_yaml() {
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml("this isn't yaml");
        assert_matches!(result, Err(ConductorConfigError::SerializationError(_)));
    }

    #[test]
    fn test_config_complete_minimal_config() {
        let yaml = r#"---
    environment_path: /path/to/env

    passphrase_service:
      type: cmd
    "#;
        let result: ConductorConfig = config_from_yaml(yaml).unwrap();
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
                keystore_path: None,
                admin_interfaces: None,
                use_dangerous_test_keystore: false,
            }
        );
    }

    #[test]
    fn test_config_complete_config() {
        observability::test_run().ok();

        let yaml = r#"---
    environment_path: /path/to/env
    use_dangerous_test_keystore: true
    signing_service_uri: ws://localhost:9001
    encryption_service_uri: ws://localhost:9002
    decryption_service_uri: ws://localhost:9003

    passphrase_service:
      type: cmd

    dpki:
      instance_id: some_id
      init_params: some_params

    admin_interfaces:
      - driver:
          type: websocket
          port: 1234

    network:
      bootstrap_service: https://bootstrap.holo.host
      transport_pool:
        - type: proxy
          sub_transport:
            type: quic
            bind_to: kitsune-quic://0.0.0.0:0
          proxy_config:
            type: local_proxy_server
            proxy_accept_config: reject_all
      tuning_params:
        gossip_loop_iteration_delay_ms: 42
        default_notify_remote_agent_count: 42
        default_notify_timeout_ms: 42
        default_rpc_single_timeout_ms: 42
        default_rpc_multi_remote_agent_count: 42
        default_rpc_multi_timeout_ms: 42
        agent_info_expires_after_ms: 42

    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        use holochain_p2p::kitsune_p2p::*;
        let mut network_config = KitsuneP2pConfig::default();
        network_config.bootstrap_service = Some(url2::url2!("https://bootstrap.holo.host"));
        network_config.transport_pool.push(TransportConfig::Proxy {
            sub_transport: Box::new(TransportConfig::Quic {
                bind_to: Some(url2::url2!("kitsune-quic://0.0.0.0:0")),
                override_host: None,
                override_port: None,
            }),
            proxy_config: ProxyConfig::LocalProxyServer {
                proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
            },
        });
        {
            let mut tuning_params = &mut network_config.tuning_params;
            tuning_params.gossip_loop_iteration_delay_ms = 42;
            tuning_params.default_notify_remote_agent_count = 42;
            tuning_params.default_notify_timeout_ms = 42;
            tuning_params.default_rpc_single_timeout_ms = 42;
            tuning_params.default_rpc_multi_remote_agent_count = 42;
            tuning_params.default_rpc_multi_timeout_ms = 42;
            tuning_params.agent_info_expires_after_ms = 42;
        }
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                use_dangerous_test_keystore: true,
                signing_service_uri: Some("ws://localhost:9001".into()),
                encryption_service_uri: Some("ws://localhost:9002".into()),
                decryption_service_uri: Some("ws://localhost:9003".into()),
                dpki: Some(DpkiConfig {
                    instance_id: "some_id".into(),
                    init_params: "some_params".into()
                }),
                passphrase_service: Some(PassphraseServiceConfig::Cmd),
                keystore_path: None,
                admin_interfaces: Some(vec![AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket { port: 1234 }
                }]),
                network: Some(network_config),
            }
        );
    }

    #[test]
    fn test_config_keystore() {
        let yaml = r#"---
    environment_path: /path/to/env
    use_dangerous_test_keystore: true
    keystore_path: /path/to/keystore

    passphrase_service:
      type: fromconfig
      passphrase: foobar
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                network: None,
                signing_service_uri: None,
                encryption_service_uri: None,
                decryption_service_uri: None,
                dpki: None,
                passphrase_service: Some(PassphraseServiceConfig::FromConfig {
                    passphrase: "foobar".into()
                }),
                keystore_path: Some(PathBuf::from("/path/to/keystore").into()),
                admin_interfaces: None,
                use_dangerous_test_keystore: true,
            }
        );
    }
}
