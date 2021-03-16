use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use std::sync::Arc;
use url2::Url2;

/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap-staging.holo.host";
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

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
    pub tuning_params: Arc<KitsuneP2pTuningParams>,
    /// The network used for connecting to other peers
    pub network_type: NetworkType,
}

impl Default for KitsuneP2pConfig {
    fn default() -> Self {
        Self {
            transport_pool: Vec::new(),
            bootstrap_service: None,
            tuning_params: Arc::new(KitsuneP2pTuningParams::default()),
            network_type: NetworkType::QuicBootstrap,
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

/// Method for connecting to other peers and broadcasting our AgentInfo
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkType {
    /// Via bootstrap server to the WAN
    QuicBootstrap,
    /// Via MDNS to the LAN
    QuicMdns,
}
