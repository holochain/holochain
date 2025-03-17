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
//!   ## Use the Holochain-provided dev-test bootstrap server.
//!   bootstrap_url: https://devtest-bootstrap-1.holochain.org
//!
//!   ## Use the Holochain-provided dev-test sbd/signalling server.
//!   signal_url: wss://devtest-sbd-1.holochain.org
//!
//!   ## Override the default WebRTC STUN configuration.
//!   ## This is OPTIONAL. If this is not specified, it will default
//!   ## to what you can see here:
//!   webrtc_config: {
//!     "iceServers": [
//!       { "urls": ["stun:devtest-stun-1.holochain.org:443"] }
//!     ]
//!   }
//! "#;
//!
//!use holochain_conductor_api::conductor::ConductorConfig;
//!
//!let _: ConductorConfig = serde_yaml::from_str(yaml).unwrap();
//! ```

use crate::conductor::process::ERROR_CODE;
use holochain_types::prelude::DbSyncStrategy;
use schemars::JsonSchema;
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

/// All the config information for the conductor
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, Default, JsonSchema)]
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
    #[serde(default)]
    pub network: NetworkConfig,

    /// Optional specification of Chain Head Coordination service URL.
    /// If set, each cell's commit workflow will include synchronizing with the specified CHC service.
    /// If you don't know what this means, leave this setting alone (as `None`)
    #[schemars(default, schema_with = "holochain_util::jsonschema::url2_schema")]
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

    /// Tracing scope.
    pub tracing_scope: Option<String>,
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

    /// Get the tracing scope from the conductor config.
    pub fn tracing_scope(&self) -> Option<String> {
        self.tracing_scope.clone()
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
        self.network.bootstrap_url == url2::url2!("rendezvous:")
    }
}

/// All the network config information for the conductor.
#[derive(Clone, Deserialize, Serialize, Debug, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub struct NetworkConfig {
    /// The Kitsune2 bootstrap server to use for WAN discovery.
    #[schemars(schema_with = "holochain_util::jsonschema::url2_schema")]
    pub bootstrap_url: url2::Url2,

    /// The Kitsune2 sbd server to use for webrtc signalling.
    #[schemars(schema_with = "holochain_util::jsonschema::url2_schema")]
    pub signal_url: url2::Url2,

    /// The Kitsune2 webrtc_config to use for connecting to peers.
    pub webrtc_config: Option<serde_json::Value>,

    /// Use this advanced field to directly configure kitsune2.
    ///
    /// The above options actually just set specific values in this config.
    /// Use only if you know what you are doing!
    pub advanced: Option<serde_json::Value>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bootstrap_url: url2::Url2::parse("https://devtest-bootstrap-1.holochain.org"),
            signal_url: url2::Url2::parse("wss://devtest-sbd-1.holochain.org"),
            webrtc_config: None,
            advanced: None,
        }
    }
}

impl NetworkConfig {
    /// Set the gossip interval.
    #[cfg(feature = "test-utils")]
    pub fn with_gossip_initiate_interval_ms(mut self, initiate_interval_ms: u32) -> Self {
        self.insert_into_config(|module_config| {
            Self::insert_module_config(
                module_config,
                "k2Gossip",
                "initiateIntervalMs",
                serde_json::Value::Number(serde_json::Number::from(initiate_interval_ms)),
            )?;

            Ok(())
        })
        .unwrap();

        self
    }

    /// Set the gossip min initiate interval.
    #[cfg(feature = "test-utils")]
    pub fn with_gossip_min_initiate_interval_ms(mut self, min_initiate_interval_ms: u32) -> Self {
        self.insert_into_config(|module_config| {
            Self::insert_module_config(
                module_config,
                "k2Gossip",
                "minInitiateIntervalMs",
                serde_json::Value::Number(serde_json::Number::from(min_initiate_interval_ms)),
            )?;

            Ok(())
        })
        .unwrap();

        self
    }

    /// Set the gossip round timeout.
    #[cfg(feature = "test-utils")]
    pub fn with_gossip_round_timeout_ms(mut self, round_timeout_ms: u32) -> Self {
        self.insert_into_config(|module_config| {
            Self::insert_module_config(
                module_config,
                "k2Gossip",
                "roundTimeoutMs",
                serde_json::Value::Number(serde_json::Number::from(round_timeout_ms)),
            )?;

            Ok(())
        })
        .unwrap();

        self
    }

    /// Convert the network config to a K2 config object.
    ///
    /// Values that are set directly on the network config are merged into the [`NetworkConfig::advanced`] field.
    pub fn to_k2_config(&self) -> ConductorConfigResult<serde_json::Value> {
        let mut working = self
            .advanced
            .clone()
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

        if let Some(module_config) = working.as_object_mut() {
            Self::insert_module_config(
                module_config,
                "coreBootstrap",
                "serverUrl",
                serde_json::Value::String(self.bootstrap_url.as_str().into()),
            )?;

            Self::insert_module_config(
                module_config,
                "tx5Transport",
                "serverUrl",
                serde_json::Value::String(self.signal_url.as_str().into()),
            )?;

            // TODO nowhere to put the webrtc config in K2 yet!
        } else {
            return Err(ConductorConfigError::InvalidNetworkConfig(
                "advanced field must be an object".to_string(),
            ));
        }

        Ok(working)
    }

    fn insert_into_config(
        &mut self,
        mutator: impl Fn(&mut serde_json::Map<String, serde_json::Value>) -> ConductorConfigResult<()>,
    ) -> ConductorConfigResult<()> {
        if self.advanced.is_none() {
            self.advanced = Some(serde_json::Value::Object(Default::default()));
        }

        if let Some(module_config) = self
            .advanced
            .as_mut()
            .expect("Just checked")
            .as_object_mut()
        {
            mutator(module_config)?;
        }

        Ok(())
    }

    // Helper function for injecting a key-value pair into a module's configuration
    fn insert_module_config(
        module_config: &mut serde_json::Map<String, serde_json::Value>,
        module: &str,
        key: &str,
        value: serde_json::Value,
    ) -> ConductorConfigResult<()> {
        if let Some(module_config) = module_config.get_mut(module) {
            if let Some(module_config) = module_config.as_object_mut() {
                if module_config.contains_key(key) {
                    tracing::warn!("The {} module configuration contains a '{}' field, which is being overwritten", module, key);
                }

                // The config for this module exists and is an object, insert the key-value pair
                module_config.insert(key.into(), value);
            } else {
                // The configuration for this module exists, but isn't an object
                return Err(ConductorConfigError::InvalidNetworkConfig(format!(
                    "advanced.{} field must be an object",
                    module
                )));
            }
        } else {
            // The config for this module isn't set at all, so we need to insert it
            module_config.insert(
                module.into(),
                serde_json::json!({
                    key: value,
                }),
            );
        }

        Ok(())
    }
}

/// Tuning parameters to adjust the behaviour of the conductor.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
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
    /// Only publish a DhtOp once during this interval. This allows for triggering the publish workflow
    /// frequently without flooding the network with spurious publishes.
    ///
    /// Default: 5 minutes
    pub min_publish_interval: Option<std::time::Duration>,
}

impl ConductorTuningParams {
    /// Create a new [`ConductorTuningParams`] with all values missing, which will cause the defaults to be used.
    pub fn new() -> Self {
        Self {
            sys_validation_retry_delay: None,
            countersigning_resolution_retry_delay: None,
            countersigning_resolution_retry_limit: None,
            min_publish_interval: None,
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

    /// Get the current value of `min_publish_interval` or its default value.
    pub fn min_publish_interval(&self) -> std::time::Duration {
        self.min_publish_interval
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
            min_publish_interval: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::websocket::AllowedOrigins;
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
                network: NetworkConfig::default(),
                dpki: DpkiConfig::default(),
                keystore: KeystoreConfig::DangerTestKeystore,
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::default(),
                #[cfg(feature = "chc")]
                chc_url: None,
                tuning_params: None,
                tracing_scope: None,
            }
        );
    }

    #[cfg(not(feature = "unstable-dpki"))]
    #[test]
    #[allow(clippy::field_reassign_with_default)]
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
      dna_path: ~
      network_seed: ""
      no_dpki: true

    admin_interfaces:
      - driver:
          type: websocket
          port: 1234
          allowed_origins: "*"

    network:
      bootstrap_url: https://test-boot.tld
      signal_url: wss://test-sig.tld
      webrtc_config: {
        "iceServers": [
          { "urls": ["stun:test-stun.tld:443"] },
        ]
      }
      advanced: {
        "my": {
          "totally": {
            "random": {
              "advanced": {
                "config": true
              }
            }
          }
        }
      }

    db_sync_strategy: Fast
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        let mut network_config = NetworkConfig::default();
        network_config.bootstrap_url = url2::url2!("https://test-boot.tld");
        network_config.signal_url = url2::url2!("wss://test-sig.tld");
        network_config.webrtc_config = Some(serde_json::json!({
            "iceServers": [
                { "urls": ["stun:test-stun.tld:443"] },
            ]
        }));
        network_config.advanced = Some(serde_json::json!({
            "my": {
                "totally": {
                    "random": {
                        "advanced": {
                            "config": true,
                        }
                    }
                }
            }
        }));

        pretty_assertions::assert_eq!(
            result.unwrap(),
            ConductorConfig {
                tracing_override: None,
                data_root_path: Some(PathBuf::from("/path/to/env").into()),
                device_seed_lair_tag: None,
                danger_generate_throwaway_device_seed: false,
                dpki: DpkiConfig::disabled(),
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
                tracing_scope: None,
            }
        );
    }

    #[cfg(feature = "unstable-dpki")]
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
      bootstrap_url: https://test-boot.tld
      signal_url: wss://test-sig.tld

    db_sync_strategy: Fast
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        let network_config = NetworkConfig {
            bootstrap_url: url2::url2!("https://test-boot.tld"),
            signal_url: url2::url2!("wss://test-sig.tld"),
            ..Default::default()
        };
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
                tracing_scope: None,
            }
        );
    }

    #[test]
    fn test_config_new_lair_keystore() {
        let yaml = r#"---
    data_root_path: /path/to/env
    keystore_path: /path/to/keystore
    keystore:
      type: lair_server
      connection_url: "unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ"
    "#;
        let result: ConductorConfigResult<ConductorConfig> = config_from_yaml(yaml);
        pretty_assertions::assert_eq!(
            result.unwrap(),
            ConductorConfig {
                tracing_override: None,
                data_root_path: Some(PathBuf::from("/path/to/env").into()),
                device_seed_lair_tag: None,
                danger_generate_throwaway_device_seed: false,
                network: NetworkConfig::default(),
                dpki: Default::default(),
                keystore: KeystoreConfig::LairServer {
                    connection_url: url2::url2!("unix:///var/run/lair-keystore/socket?k=EcRDnP3xDIZ9Rk_1E-egPE0mGZi5CcszeRxVkb2QXXQ"),
                },
                admin_interfaces: None,
                db_sync_strategy: DbSyncStrategy::Resilient,
                #[cfg(feature = "chc")]
                chc_url: None,
                tuning_params: None,
                tracing_scope: None,
            }
        );
    }

    #[test]
    fn default_network_config_accepted_by_k2() {
        let network_config = NetworkConfig::default();
        let k2_config = network_config.to_k2_config().unwrap();

        let builder = kitsune2_core::default_test_builder()
            .with_default_config()
            .unwrap();
        builder.config.set_module_config(&k2_config).unwrap();
        builder.validate_config().unwrap();
    }

    #[test]
    fn network_config_preserves_advanced_overrides() {
        let network_config = NetworkConfig {
            advanced: Some(serde_json::json!({
                "coreBootstrap": {
                    "backoffMinMs": "3500",
                },
                "tx5Transport": {
                    "timeoutS": "10",
                },
                "coreSpace": {
                    "reSignFreqMs": "1000",
                }
            })),
            ..Default::default()
        };

        let k2_config = network_config.to_k2_config().unwrap();

        let builder = kitsune2_core::default_test_builder()
            .with_default_config()
            .unwrap();
        builder.config.set_module_config(&k2_config).unwrap();
        builder.validate_config().unwrap();
        assert_eq!(
            k2_config,
            serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": "https://devtest-bootstrap-1.holochain.org/",
                    "backoffMinMs": "3500",
                },
                "tx5Transport": {
                    "serverUrl": "wss://devtest-sbd-1.holochain.org/",
                    "timeoutS": "10",
                },
                "coreSpace": {
                    "reSignFreqMs": "1000",
                }
            })
        )
    }

    #[test]
    fn network_config_overrides_conflicting_advanced_fields() {
        let network_config = NetworkConfig {
            advanced: Some(serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": "https://something-else.net",
                },
                "tx5Transport": {
                    "serverUrl": "wss://sbd.nowhere.net",
                },
            })),
            ..Default::default()
        };

        let k2_config = network_config.to_k2_config().unwrap();

        let builder = kitsune2_core::default_test_builder()
            .with_default_config()
            .unwrap();
        builder.config.set_module_config(&k2_config).unwrap();
        builder.validate_config().unwrap();

        assert_eq!(
            k2_config,
            serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": "https://devtest-bootstrap-1.holochain.org/",
                },
                "tx5Transport": {
                    "serverUrl": "wss://devtest-sbd-1.holochain.org/",
                },
            })
        )
    }

    #[test]
    fn tune_kitsune_params_for_testing() {
        let network_config = NetworkConfig::default()
            .with_gossip_round_timeout_ms(100)
            .with_gossip_initiate_interval_ms(200)
            .with_gossip_min_initiate_interval_ms(300);

        let k2_config = network_config.to_k2_config().unwrap();

        let builder = kitsune2_core::default_test_builder()
            .with_default_config()
            .unwrap();
        builder.config.set_module_config(&k2_config).unwrap();
        builder.validate_config().unwrap();

        assert_eq!(
            k2_config,
            serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": "https://devtest-bootstrap-1.holochain.org/",
                },
                "tx5Transport": {
                    "serverUrl": "wss://devtest-sbd-1.holochain.org/",
                },
                "k2Gossip": {
                    "roundTimeoutMs": 100,
                    "initiateIntervalMs": 200,
                    "minInitiateIntervalMs": 300,
                }
            })
        )
    }
}
