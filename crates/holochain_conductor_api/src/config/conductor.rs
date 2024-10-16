#![deny(missing_docs)]
//! This module is used to configure the conductor.
//!
//! #### Example minimum conductor config:
//!
//! ```rust
//! let yaml = r#"---
//!
//! ## Configure the keystore to be used.
//! keystore:
//!
//!   ## Use an in-process keystore with default database location.
//!   type: lair_server_in_proc
//!
//! ## Configure an admin WebSocket interface at a specific port.
//! admin_interfaces:
//!   - driver:
//!       type: websocket
//!       port: 1234
//!       allowed_origins: "*"
//!
//! ## Configure the network.
//! network:
//!
//!   ## Use the Holo-provided default production bootstrap server.
//!   bootstrap_service: https://bootstrap.holo.host
//!
//!   ## This currently has no effect on functionality but is required. Please just include as-is for now.
//!   network_type: quic_bootstrap
//!
//!   ## Setup a specific network configuration.
//!   transport_pool:
//!     ## Use WebRTC, which is the only option for now.
//!     - type: webrtc
//!
//!       ## Use the Holo-provided default production sbd (signal) server.
//!       ## `signal_url` is REQUIRED.
//!       signal_url: wss://sbd-0.main.infra.holo.host
//!
//!       ## Override the default WebRTC STUN configuration.
//!       ## This is OPTIONAL. If this is not specified, it will default
//!       ## to what you can see here:
//!       webrtc_config: {
//!         "iceServers": [
//!           { "urls": ["stun:stun-0.main.infra.holo.host:443"] },
//!           { "urls": ["stun:stun-1.main.infra.holo.host:443"] }
//!         ]
//!       }
//! "#;
//!
//!use holochain_conductor_api::conductor::ConductorConfig;
//!
//!let _: ConductorConfig = serde_yaml::from_str(yaml).unwrap();
//! ```

use crate::conductor::process::ERROR_CODE;
use holochain_types::prelude::DbSyncStrategy;
use kitsune_p2p_types::config::{KitsuneP2pConfig, KitsuneP2pTuningParams};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

mod admin_interface_config;
mod dpki_config;
#[allow(missing_docs)]
mod error;
mod keystore_config;
/// Defines subdirectories of the config directory.
pub mod paths;
pub mod process;
//mod logger_config;
//mod signal_config;

pub use super::*;
pub use dpki_config::DpkiConfig;
//pub use logger_config::LoggerConfig;
pub use error::*;
pub use keystore_config::KeystoreConfig;
//pub use signal_config::SignalConfig;
use std::path::Path;

use crate::config::conductor::paths::DataRootPath;

// TODO change types from "stringly typed" to Url2
/// All the config information for the conductor
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq)]
pub struct ConductorConfig {
    /// Override the environment specified tracing config.
    #[serde(default)]
    pub tracing_override: Option<String>,

    /// The path to the data root for this conductor;
    /// This can be `None` while building up the config programatically but MUST
    /// be set by the time the config is used to build a conductor.
    /// The database and compiled wasm directories are derived from this path.
    pub data_root_path: Option<DataRootPath>,

    /// The lair tag used to refer to the "device seed" which was used to generate
    /// the AgentPubKey for the DPKI cell.
    ///
    /// This must not be changed once the conductor has been started for the first time.
    pub device_seed_lair_tag: Option<String>,

    /// If set, and if there is no seed in lair at the tag specified in `device_seed_lair_tag`,
    /// the conductor will create a random seed and store it in lair at the specified tag.
    /// This should only be used for test or throwaway environments, because this device seed
    /// can never be regenerated, which defeats the purpose of having a device seed in the first place.
    ///
    /// If `device_seed_lair_tag` is not set, this setting has no effect.
    #[serde(default)]
    pub danger_generate_throwaway_device_seed: bool,

    /// Define how Holochain conductor will connect to a keystore.
    #[serde(default)]
    pub keystore: KeystoreConfig,

    /// DPKI config for this conductor. This setting must not change once the conductor has been
    /// started for the first time.
    ///  
    /// If `dna_path` is present, the DNA file at this path will be used to install the DPKI service upon first conductor startup.
    /// If not present, the Deepkey DNA specified by the `holochain_deepkey_dna` crate and built into Holochain, will be used instead.
    #[serde(default)]
    pub dpki: DpkiConfig,

    /// Setup admin interfaces to control this conductor through a websocket connection.
    pub admin_interfaces: Option<Vec<AdminInterfaceConfig>>,

    /// Optional config for the network module.
    pub network: KitsuneP2pConfig,

    /// Optional specification of Chain Head Coordination service URL.
    /// If set, each cell's commit workflow will include synchronizing with the specified CHC service.
    /// If you don't know what this means, leave this setting alone (as `None`)
    #[serde(default)]
    #[cfg(feature = "chc")]
    pub chc_url: Option<url2::Url2>,

    /// Override the default database synchronous strategy.
    ///
    /// See [sqlite documentation] for information about database sync levels.
    /// See [`DbSyncStrategy`] for details.
    /// This is best left at its default value unless you know what you
    /// are doing.
    ///
    /// [sqlite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
    #[serde(default)]
    pub db_sync_strategy: DbSyncStrategy,

    /// Tuning parameters to adjust the behaviour of the conductor.
    #[serde(default)]
    pub tuning_params: Option<ConductorTuningParams>,
}

/// Helper function to load a config from a YAML string.
fn config_from_yaml<T>(yaml: &str) -> ConductorConfigResult<T>
where
    T: DeserializeOwned,
{
    serde_yaml::from_str(yaml).map_err(ConductorConfigError::SerializationError)
}

impl ConductorConfig {
    /// The most minimal config, which will not work
    pub fn empty() -> Self {
        Self {
            tracing_override: None,
            data_root_path: None,
            keystore: KeystoreConfig::default(),
            dpki: DpkiConfig::testing(),
            admin_interfaces: None,
            network: KitsuneP2pConfig::empty(),
            db_sync_strategy: DbSyncStrategy::default(),
            tuning_params: None,
            device_seed_lair_tag: None,
            danger_generate_throwaway_device_seed: false,
            #[cfg(feature = "chc")]
            chc_url: None,
        }
    }

    /// Create a config using the testing network config,
    /// testing DPKI, and default values for everything else.
    pub fn testing() -> Self {
        Self {
            network: KitsuneP2pConfig::testing(),
            dpki: DpkiConfig::testing(),
            ..ConductorConfig::empty()
        }
    }

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

    /// Get tuning params for this config (default if not set)
    pub fn kitsune_tuning_params(&self) -> KitsuneP2pTuningParams {
        self.network.tuning_params.clone()
    }

    /// Get the tracing scope from the network config
    pub fn tracing_scope(&self) -> Option<String> {
        self.network.tracing_scope.clone()
    }

    /// Get the data directory for this config or say something nice and die.
    pub fn data_root_path_or_die(&self) -> DataRootPath {
        match &self.data_root_path {
            Some(path) => path.clone(),
            None => {
                println!(
                    "
                    The conductor config does not contain a data_root_path. Please check and fix the
                    config file. Details:

                        Missing field `data_root_path`",
                );
                std::process::exit(ERROR_CODE);
            }
        }
    }

    /// Get the conductor tuning params for this config (default if not set)
    pub fn conductor_tuning_params(&self) -> ConductorTuningParams {
        self.tuning_params.clone().unwrap_or_default()
    }

    /// Check if the config is set to use a rendezvous bootstrap server
    pub fn has_rendezvous_bootstrap(&self) -> bool {
        self.network.bootstrap_service == Some(url2::url2!("rendezvous:"))
    }
}

/// Tuning parameters to adjust the behaviour of the conductor.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ConductorTuningParams {
    /// The delay between retries of sys validation when there are missing dependencies waiting to be found on the DHT.
    ///
    /// Default: 10 seconds
    pub sys_validation_retry_delay: Option<std::time::Duration>,
    /// The delay between retries attempts at resolving failed countersigning sessions.
    ///
    /// This is potentially a very heavy operation because it has to gather information from the network,
    /// so it is recommended not to set this too low.
    ///
    /// Default: 5 minutes
    pub countersigning_resolution_retry_delay: Option<std::time::Duration>,
    /// The maximum number of times that Holochain should attempt to resolve a failed countersigning session.
    ///
    /// Note that this *only* applies to sessions that fail through a timeout. Sessions that fail because
    /// of a conductor crash or otherwise will not be limited by this value. This is a safety measure to
    /// make it less likely that timeout leads to a wrong decision because of a temporary network issue.
    ///
    /// Holochain will always try once, whatever value you set. The possible values for this setting are:
    /// - `None`: Not set, then Holochain will just make a single attempt and then consider the session failed
    ///    if it can't make a decision.
    /// - `Some(0)`: Holochain will treat this the same as a session that failed after a crash. It will retry
    ///   until it can make a decision or until the user forces a decision.
    /// - `Some(n)`, n > 0: Holochain will retry `n` times, including the required first attempt. If
    ///   it can't make a decision after `n` retries, it will consider the session failed.
    pub countersigning_resolution_retry_limit: Option<usize>,
}

impl ConductorTuningParams {
    /// Create a new [`ConductorTuningParams`] with all values missing, which will cause the defaults to be used.
    pub fn new() -> Self {
        Self {
            sys_validation_retry_delay: None,
            countersigning_resolution_retry_delay: None,
            countersigning_resolution_retry_limit: None,
        }
    }

    /// Get the current value of `sys_validation_retry_delay` or its default value.
    pub fn sys_validation_retry_delay(&self) -> std::time::Duration {
        self.sys_validation_retry_delay
            .unwrap_or_else(|| std::time::Duration::from_secs(10))
    }

    /// Get the current value of `countersigning_resolution_retry_delay` or its default value.
    pub fn countersigning_resolution_retry_delay(&self) -> std::time::Duration {
        self.countersigning_resolution_retry_delay
            .unwrap_or_else(|| std::time::Duration::from_secs(60 * 5))
    }
}

impl Default for ConductorTuningParams {
    fn default() -> Self {
        let empty = Self::new();
        Self {
            sys_validation_retry_delay: Some(empty.sys_validation_retry_delay()),
            countersigning_resolution_retry_delay: Some(
                empty.countersigning_resolution_retry_delay(),
            ),
            countersigning_resolution_retry_limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::websocket::AllowedOrigins;
    use kitsune_p2p_types::config::TransportConfig;
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
    data_root_path: /path/to/env
    network:
      transport_pool: []
    keystore:
      type: danger_test_keystore
    "#;
        let result: ConductorConfig = config_from_yaml(yaml).unwrap();
        pretty_assertions::assert_eq!(
            result,
            ConductorConfig {
                tracing_override: None,
                data_root_path: Some(PathBuf::from("/path/to/env").into()),
                device_seed_lair_tag: None,
                danger_generate_throwaway_device_seed: false,
                network: KitsuneP2pConfig::empty(),
                dpki: DpkiConfig::testing(),
                keystore: KeystoreConfig::DangerTestKeystore,
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::default(),
                #[cfg(feature = "chc")]
                chc_url: None,
                tuning_params: None,
            }
        );
    }

    #[test]
    fn test_config_complete_config() {
        holochain_trace::test_run();

        let yaml = r#"---
    data_root_path: /path/to/env
    signing_service_uri: ws://localhost:9001
    encryption_service_uri: ws://localhost:9002
    decryption_service_uri: ws://localhost:9003

    keystore:
      type: lair_server_in_proc

    dpki:
      dna_path: path/to/dna.dna
      network_seed: "deepkey-main"
      device_seed_lair_tag: "device-seed"

    admin_interfaces:
      - driver:
          type: websocket
          port: 1234
          allowed_origins: "*"

    network:
      bootstrap_service: https://bootstrap-staging.holo.host
      transport_pool:
        - type: webrtc
          signal_url: wss://sbd-0.main.infra.holo.host
          webrtc_config: {
            "iceServers": [
              { "urls": ["stun:stun-0.main.infra.holo.host:443"] },
              { "urls": ["stun:stun-1.main.infra.holo.host:443"] }
            ]
          }
      tuning_params:
        gossip_loop_iteration_delay_ms: 42
        default_rpc_single_timeout_ms: 42
        default_rpc_multi_remote_agent_count: 42
        default_rpc_multi_remote_request_grace_ms: 42
        agent_info_expires_after_ms: 42
        tls_in_mem_session_storage: 42
        proxy_keepalive_ms: 42
        proxy_to_expire_ms: 42
        tx5_min_ephemeral_udp_port: 40000
        tx5_max_ephemeral_udp_port: 40255
      network_type: quic_bootstrap

    db_sync_strategy: Fast
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        let mut network_config = KitsuneP2pConfig::empty();
        network_config.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host"));
        network_config.transport_pool.push(TransportConfig::WebRTC {
            signal_url: "wss://sbd-0.main.infra.holo.host".into(),
            webrtc_config: Some(serde_json::json!({
              "iceServers": [
                { "urls": ["stun:stun-0.main.infra.holo.host:443"] },
                { "urls": ["stun:stun-1.main.infra.holo.host:443"] }
              ]
            })),
        });
        let mut tuning_params =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning_params.gossip_loop_iteration_delay_ms = 42;
        tuning_params.default_rpc_single_timeout_ms = 42;
        tuning_params.default_rpc_multi_remote_agent_count = 42;
        tuning_params.default_rpc_multi_remote_request_grace_ms = 42;
        tuning_params.agent_info_expires_after_ms = 42;
        tuning_params.tls_in_mem_session_storage = 42;
        tuning_params.proxy_keepalive_ms = 42;
        tuning_params.proxy_to_expire_ms = 42;
        tuning_params.tx5_min_ephemeral_udp_port = 40000;
        tuning_params.tx5_max_ephemeral_udp_port = 40255;
        network_config.tuning_params = std::sync::Arc::new(tuning_params);
        pretty_assertions::assert_eq!(
            result.unwrap(),
            ConductorConfig {
                tracing_override: None,
                data_root_path: Some(PathBuf::from("/path/to/env").into()),
                device_seed_lair_tag: None,
                danger_generate_throwaway_device_seed: false,
                dpki: DpkiConfig::production(Some("path/to/dna.dna".into())),
                keystore: KeystoreConfig::LairServerInProc { lair_root: None },
                admin_interfaces: Some(vec![AdminInterfaceConfig {
                    driver: InterfaceDriver::Websocket {
                        port: 1234,
                        allowed_origins: AllowedOrigins::Any
                    }
                }]),
                network: network_config,
                db_sync_strategy: DbSyncStrategy::Fast,
                #[cfg(feature = "chc")]
                chc_url: None,
                tuning_params: None,
            }
        );
    }

    #[test]
    fn test_config_new_lair_keystore() {
        let yaml = r#"---
    data_root_path: /path/to/env
    keystore_path: /path/to/keystore
    network:
      transport_pool: []
    keystore:
      type: lair_server
      connection_url: "unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ"
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        assert_eq!(
            result.unwrap(),
            ConductorConfig {
                tracing_override: None,
                data_root_path: Some(PathBuf::from("/path/to/env").into()),
                device_seed_lair_tag: None,
                danger_generate_throwaway_device_seed: false,
                network: KitsuneP2pConfig::empty(),
                dpki: Default::default(),
                keystore: KeystoreConfig::LairServer {
                    connection_url: url2::url2!("unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ"),
                },
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::Resilient,
                #[cfg(feature = "chc")]
                chc_url: None,
                tuning_params: None,
            }
        );
    }
}
