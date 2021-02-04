use ghost_actor::dependencies::tracing;
use std::collections::HashMap;
use url2::Url2;

/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap.holo.host";
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

/// Network tuning parameters.
/// This is serialized carefully so all the values can be represented
/// as strings in YAML - and we will be able to proceed with a printed
/// warning for tuning params that are removed, but still specified in
/// configs.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
#[allow(missing_docs)]
pub struct KitsuneP2pTuningParams {
    pub gossip_loop_iteration_delay_ms: u32,
    pub default_notify_remote_agent_count: u32,
    pub default_notify_timeout_ms: u32,
    pub default_rpc_single_timeout_ms: u32,
    pub default_rpc_multi_remote_agent_count: u32,
    pub default_rpc_multi_timeout_ms: u32,
    pub agent_info_expires_after_ms: u32,
}

impl Default for KitsuneP2pTuningParams {
    fn default() -> Self {
        Self {
            gossip_loop_iteration_delay_ms: 10,
            default_notify_remote_agent_count: 5,
            default_notify_timeout_ms: 1000,
            default_rpc_single_timeout_ms: 2000,
            default_rpc_multi_remote_agent_count: 2,
            default_rpc_multi_timeout_ms: 2000,
            agent_info_expires_after_ms: 1000 * 60 * 20, // 20 minutes
        }
    }
}

impl serde::Serialize for KitsuneP2pTuningParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut m = serializer.serialize_map(Some(1))?;
        m.serialize_entry(
            "gossip_loop_iteration_delay_ms",
            &format!("{}", self.gossip_loop_iteration_delay_ms),
        )?;
        m.serialize_entry(
            "default_notify_remote_agent_count",
            &format!("{}", self.default_notify_remote_agent_count),
        )?;
        m.serialize_entry(
            "default_notify_timeout_ms",
            &format!("{}", self.default_notify_timeout_ms),
        )?;
        m.serialize_entry(
            "default_rpc_single_timeout_ms",
            &format!("{}", self.default_rpc_single_timeout_ms),
        )?;
        m.serialize_entry(
            "default_rpc_multi_remote_agent_count",
            &format!("{}", self.default_rpc_multi_remote_agent_count),
        )?;
        m.serialize_entry(
            "default_rpc_multi_timeout_ms",
            &format!("{}", self.default_rpc_multi_timeout_ms),
        )?;
        m.serialize_entry(
            "agent_info_expires_after_ms",
            &format!("{}", self.agent_info_expires_after_ms),
        )?;
        m.end()
    }
}

impl<'de> serde::Deserialize<'de> for KitsuneP2pTuningParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let result = <HashMap<String, String>>::deserialize(deserializer)?;
        let mut out = KitsuneP2pTuningParams::default();
        for (k, v) in result.into_iter() {
            match k.as_str() {
                "gossip_loop_iteration_delay_ms" => match v.parse::<u32>() {
                    Ok(v) => out.gossip_loop_iteration_delay_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "default_notify_remote_agent_count" => match v.parse::<u32>() {
                    Ok(v) => out.default_notify_remote_agent_count = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "default_notify_timeout_ms" => match v.parse::<u32>() {
                    Ok(v) => out.default_notify_timeout_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "default_rpc_single_timeout_ms" => match v.parse::<u32>() {
                    Ok(v) => out.default_rpc_single_timeout_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "default_rpc_multi_remote_agent_count" => match v.parse::<u32>() {
                    Ok(v) => out.default_rpc_multi_remote_agent_count = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "default_rpc_multi_timeout_ms" => match v.parse::<u32>() {
                    Ok(v) => out.default_rpc_multi_timeout_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "agent_info_expires_after_ms" => match v.parse::<u32>() {
                    Ok(v) => out.agent_info_expires_after_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                _ => tracing::warn!("INVALID TUNING PARAM: '{}'", k),
            }
        }
        Ok(out)
    }
}

/// Configure the kitsune actor
#[non_exhaustive]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct KitsuneP2pConfig {
    /// list of sub-transports to be included in this pool
    pub transport_pool: Vec<TransportConfig>,
    /// The service used for peers to discover each before they are peers.
    pub bootstrap_service: Option<Url2>,
    /// Network tuning parameters. These are managed loosely,
    /// as they are subject to change. If you specify a tuning parameter
    /// that no longer exists, or a value that does not parse,
    /// a warning will be printed in the tracing log.
    #[serde(default)]
    pub tuning_params: KitsuneP2pTuningParams,
}

impl Default for KitsuneP2pConfig {
    fn default() -> Self {
        Self {
            transport_pool: Vec::new(),
            bootstrap_service: None,
            tuning_params: KitsuneP2pTuningParams::default(),
        }
    }
}

/// Configure the network bindings for underlying kitsune transports
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    /// A transport that uses the local memory transport protocol
    /// (this is mainly for testing).
    Mem {},
    /// A transport that uses the QUIC protocol
    Quic {
        /// To which network interface / port should we bind?
        /// Default: "kitsune-quic://0.0.0.0:0".
        bind_to: Option<Url2>,

        /// If you have port-forwarding set up,
        /// or wish to apply a vanity domain name,
        /// you may need to override the local NIC ip.
        /// Default: None = use NIC ip.
        override_host: Option<String>,

        /// If you have port-forwarding set up,
        /// you may need to override the local NIC port.
        /// Default: None = use NIC port.
        override_port: Option<u16>,
    },
    /// A transport that tls tunnels through a sub-transport (ALPN kitsune-proxy/0)
    Proxy {
        /// The 'Proxy' transport is a wrapper around a sub-transport
        /// We also need to define the sub-transport.
        sub_transport: Box<TransportConfig>,

        /// Determines whether we wish to:
        /// - proxy through a remote
        /// - be a proxy server for others
        /// - be directly addressable, but not proxy for others
        proxy_config: ProxyConfig,
    },
}

/// Proxy configuration options
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProxyConfig {
    /// We want to be hosted at a remote proxy location.
    RemoteProxyClient {
        /// The remote proxy url to be hosted at
        proxy_url: Url2,
    },

    /// We want to be a proxy server for others.
    /// (We can also deny all proxy requests for something in-between).
    LocalProxyServer {
        /// Accept proxy request options
        /// Default: None = reject all proxy requests
        proxy_accept_config: Option<ProxyAcceptConfig>,
    },
}

/// Whether we are willing to proxy on behalf of others
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyAcceptConfig {
    /// We will accept all requests to proxy for remotes
    AcceptAll,

    /// We will reject all requests to proxy for remotes
    RejectAll,
}
