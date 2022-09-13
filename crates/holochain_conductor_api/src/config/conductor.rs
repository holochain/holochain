#![deny(missing_docs)]
//! This module is used to configure the conductor

use holochain_types::db::DbSyncStrategy;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

mod admin_interface_config;
mod dpki_config;
#[allow(missing_docs)]
mod error;
mod keystore_config;
pub mod paths;
//mod logger_config;
//mod signal_config;
pub use paths::DatabaseRootPath;

pub use super::*;
pub use dpki_config::DpkiConfig;
//pub use logger_config::LoggerConfig;
pub use error::*;
pub use keystore_config::KeystoreConfig;
//pub use signal_config::SignalConfig;
use std::path::Path;

// TODO change types from "stringly typed" to Url2
/// All the config information for the conductor
#[derive(Clone, Deserialize, Serialize, Default, Debug, PartialEq)]
pub struct ConductorConfig {
    /// The path to the database for this conductor;
    /// if omitted, chooses a default path.
    pub environment_path: DatabaseRootPath,

    /// Define how Holochain conductor will connect to a keystore.
    #[serde(default)]
    pub keystore: KeystoreConfig,

    /// Optional DPKI configuration if conductor is using a DPKI app to initalize and manage
    /// keys for new instances.
    pub dpki: Option<DpkiConfig>,

    /// Setup admin interfaces to control this conductor through a websocket connection.
    pub admin_interfaces: Option<Vec<AdminInterfaceConfig>>,

    /// Optional config for the network module.
    pub network: Option<holochain_p2p::kitsune_p2p::KitsuneP2pConfig>,

    #[serde(default)]
    /// Override the default database synchronous strategy.
    ///
    /// See [sqlite documentation] for information about database sync levels.
    /// See [`DbSyncStrategy`] for details.
    /// This is best left at its default value unless you know what you
    /// are doing.
    ///
    /// [sqlite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
    pub db_sync_strategy: DbSyncStrategy,
    //
    //
    // Which signals to emit
    // TODO: it's an open question whether signal config is stateful or not, i.e. whether it belongs here.
    // pub signals: SignalConfig,
}

/// Helper function to load a config from a YAML string.
fn config_from_yaml<T>(yaml: &str) -> ConductorConfigResult<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(yaml).map_err(ConductorConfigError::SerializationError)
}

impl ConductorConfig {
    /// Create a conductor config from a YAML file path.
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

    keystore:
      type: danger_test_keystore
    "#;
        let result: ConductorConfig = config_from_yaml(yaml).unwrap();
        assert_eq!(
            result,
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                network: None,
                dpki: None,
                keystore: KeystoreConfig::DangerTestKeystore,
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::default(),
            }
        );
    }

    #[test]
    fn test_config_complete_config() {
        observability::test_run().ok();

        let yaml = r#"---
    environment_path: /path/to/env
    signing_service_uri: ws://localhost:9001
    encryption_service_uri: ws://localhost:9002
    decryption_service_uri: ws://localhost:9003

    keystore:
      type: lair_server_in_proc

    dpki:
      instance_id: some_id
      init_params: some_params

    admin_interfaces:
      - driver:
          type: websocket
          port: 1234

    network:
      bootstrap_service: https://bootstrap-staging.holo.host
      transport_pool:
        - type: proxy
          sub_transport:
            type: quic
            bind_to: kitsune-quic://0.0.0.0:0
          proxy_config:
            type: remote_proxy_client_from_bootstrap
            bootstrap_url: https://bootstrap.holo.host
            fallback_proxy_url: ~
      tuning_params:
        gossip_loop_iteration_delay_ms: 42
        default_rpc_single_timeout_ms: 42
        default_rpc_multi_remote_agent_count: 42
        default_rpc_multi_remote_request_grace_ms: 42
        agent_info_expires_after_ms: 42
        tls_in_mem_session_storage: 42
        proxy_keepalive_ms: 42
        proxy_to_expire_ms: 42
      network_type: quic_bootstrap

    db_sync_strategy: Fast
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        use holochain_p2p::kitsune_p2p::*;
        let mut network_config = KitsuneP2pConfig::default();
        network_config.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host"));
        network_config.transport_pool.push(TransportConfig::Proxy {
            sub_transport: Box::new(TransportConfig::Quic {
                bind_to: Some(url2::url2!("kitsune-quic://0.0.0.0:0")),
                override_host: None,
                override_port: None,
            }),
            proxy_config: ProxyConfig::RemoteProxyClientFromBootstrap {
                bootstrap_url: url2::url2!("https://bootstrap.holo.host"),
                fallback_proxy_url: None,
            },
        });
        let mut tuning_params =
            kitsune_p2p::dependencies::kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning_params.gossip_loop_iteration_delay_ms = 42;
        tuning_params.default_rpc_single_timeout_ms = 42;
        tuning_params.default_rpc_multi_remote_agent_count = 42;
        tuning_params.default_rpc_multi_remote_request_grace_ms = 42;
        tuning_params.agent_info_expires_after_ms = 42;
        tuning_params.tls_in_mem_session_storage = 42;
        tuning_params.proxy_keepalive_ms = 42;
        tuning_params.proxy_to_expire_ms = 42;
        network_config.tuning_params = std::sync::Arc::new(tuning_params);
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                dpki: Some(DpkiConfig {
                    instance_id: "some_id".into(),
                    init_params: "some_params".into()
                }),
                keystore: KeystoreConfig::LairServerInProc { lair_root: None },
                admin_interfaces: Some(vec![AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket { port: 1234 }
                }]),
                network: Some(network_config),
                db_sync_strategy: DbSyncStrategy::Fast,
            }
        );
    }

    #[test]
    fn test_config_new_lair_keystore() {
        let yaml = r#"---
    environment_path: /path/to/env
    keystore_path: /path/to/keystore

    keystore:
      type: lair_server
      connection_url: "unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ"
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                environment_path: PathBuf::from("/path/to/env").into(),
                network: None,
                dpki: None,
                keystore: KeystoreConfig::LairServer {
                    connection_url: url2::url2!("unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ").into(),
                },
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::Fast,
            }
        );
    }
}
