use kitsune_p2p_types::config::{tuning_params_struct, KitsuneP2pTuningParams};
use kitsune_p2p_types::tx2::tx2_adapter::AdapterFactory;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::*;
use url2::Url2;

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap-staging.holo.host";

// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

pub(crate) enum KitsuneP2pTx2Backend {
    Mem,
    Quic { bind_to: TxUrl },
    Mock { mock_network: AdapterFactory },
}

pub(crate) enum KitsuneP2pTx2ProxyConfig {
    NoProxy,
    Specific(TxUrl),
    Bootstrap {
        #[allow(dead_code)]
        bootstrap_url: TxUrl,
        fallback_proxy_url: Option<TxUrl>,
    },
}

pub(crate) struct KitsuneP2pTx2Config {
    pub backend: KitsuneP2pTx2Backend,
    pub use_proxy: KitsuneP2pTx2ProxyConfig,
}

/// Configure the kitsune actor.
#[non_exhaustive]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct KitsuneP2pConfig {
    /// List of sub-transports to be included in this pool
    pub transport_pool: Vec<TransportConfig>,
    /// The service used for peers to discover each before they are peers.
    pub bootstrap_service: Option<Url2>,
    /// Network tuning parameters. These are managed loosely,
    /// as they are subject to change. If you specify a tuning parameter
    /// that no longer exists, or a value that does not parse,
    /// a warning will be printed in the tracing log.
    #[serde(default)]
    pub tuning_params: KitsuneP2pTuningParams,
    /// The network used for connecting to other peers
    pub network_type: NetworkType,
}

impl Default for KitsuneP2pConfig {
    fn default() -> Self {
        Self {
            transport_pool: Vec::new(),
            bootstrap_service: None,
            tuning_params: KitsuneP2pTuningParams::default(),
            network_type: NetworkType::QuicBootstrap,
        }
    }
}

fn cnv_bind_to(bind_to: &Option<url2::Url2>) -> TxUrl {
    match bind_to {
        Some(bind_to) => bind_to.clone().into(),
        None => "kitsune-quic://0.0.0.0:0".into(),
    }
}

impl KitsuneP2pConfig {
    /// `tx2` is currently designed to use exactly one proxy wrapped transport,
    /// so convert a bunch of the options from the previous transport
    /// paradigm into that pattern.
    pub(crate) fn to_tx2(&self) -> KitsuneResult<KitsuneP2pTx2Config> {
        use KitsuneP2pTx2ProxyConfig::*;
        match self.transport_pool.get(0) {
            Some(TransportConfig::Proxy {
                sub_transport,
                proxy_config,
            }) => {
                let backend = match &**sub_transport {
                    TransportConfig::Mem {} => KitsuneP2pTx2Backend::Mem,
                    TransportConfig::Quic { bind_to, .. } => {
                        let bind_to = cnv_bind_to(bind_to);
                        KitsuneP2pTx2Backend::Quic { bind_to }
                    }
                    _ => return Err("kitsune tx2 backend must be mem or quic".into()),
                };
                let use_proxy = match proxy_config {
                    ProxyConfig::RemoteProxyClient { proxy_url } => {
                        Specific(proxy_url.clone().into())
                    }
                    ProxyConfig::RemoteProxyClientFromBootstrap {
                        bootstrap_url,
                        fallback_proxy_url,
                    } => Bootstrap {
                        bootstrap_url: bootstrap_url.clone().into(),
                        fallback_proxy_url: fallback_proxy_url.clone().map(Into::into),
                    },
                    ProxyConfig::LocalProxyServer { .. } => NoProxy,
                };
                Ok(KitsuneP2pTx2Config { backend, use_proxy })
            }
            Some(TransportConfig::Quic { bind_to, .. }) => {
                let bind_to = cnv_bind_to(bind_to);
                Ok(KitsuneP2pTx2Config {
                    backend: KitsuneP2pTx2Backend::Quic { bind_to },
                    use_proxy: NoProxy,
                })
            }
            Some(TransportConfig::Mock { mock_network }) => Ok(KitsuneP2pTx2Config {
                backend: KitsuneP2pTx2Backend::Mock {
                    mock_network: mock_network.0.clone(),
                },
                use_proxy: NoProxy,
            }),
            None | Some(TransportConfig::Mem {}) => Ok(KitsuneP2pTx2Config {
                backend: KitsuneP2pTx2Backend::Mem,
                use_proxy: NoProxy,
            }),
        }
    }

    /// Return a copy with the tuning params altered
    pub fn tune(
        mut self,
        f: impl Fn(
            tuning_params_struct::KitsuneP2pTuningParams,
        ) -> tuning_params_struct::KitsuneP2pTuningParams,
    ) -> Self {
        let tp = (*self.tuning_params).clone();
        self.tuning_params = std::sync::Arc::new(f(tp));
        self
    }
}

/// Configure the network bindings for underlying kitsune transports.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    /// A transport that uses the local memory transport protocol
    /// (this is mainly for testing)
    Mem {},
    /// A transport that uses the QUIC protocol
    Quic {
        /// Network interface / port to bind to
        /// Default: "kitsune-quic://0.0.0.0:0"
        bind_to: Option<Url2>,

        /// If you have port-forwarding set up,
        /// or wish to apply a vanity domain name,
        /// you may need to override the local NIC IP.
        /// Default: None = use NIC IP
        override_host: Option<String>,

        /// If you have port-forwarding set up,
        /// you may need to override the local NIC port.
        /// Default: None = use NIC port
        override_port: Option<u16>,
    },
    /// A transport that TLS tunnels through a sub-transport (ALPN kitsune-proxy/0)
    Proxy {
        /// The 'Proxy' transport is a wrapper around a sub-transport.
        /// We also need to define the sub-transport.
        sub_transport: Box<TransportConfig>,

        /// Determines whether we wish to:
        /// - proxy through a remote
        /// - be a proxy server for others
        /// - be directly addressable, but not proxy for others
        proxy_config: ProxyConfig,
    },
    #[serde(skip)]
    /// A mock network for testing
    Mock {
        /// The adaptor for mocking the network
        mock_network: AdapterFactoryMock,
    },
}

#[derive(Clone)]
/// A simple wrapper around the [`AdaptorFactory`](tx2::tx2_adapter::AdapterFactory)
/// to allow implementing Debug and PartialEq.
pub struct AdapterFactoryMock(pub AdapterFactory);

impl std::fmt::Debug for AdapterFactoryMock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AdapterFactoryMock").finish()
    }
}

impl std::cmp::PartialEq for AdapterFactoryMock {
    fn eq(&self, _: &Self) -> bool {
        unimplemented!()
    }
}

impl From<AdapterFactory> for AdapterFactoryMock {
    fn from(adaptor_factory: AdapterFactory) -> Self {
        Self(adaptor_factory)
    }
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

    /// We want to be hosted at a remote proxy location.
    /// We'd like to fetch a proxy list from a bootstrap server,
    /// with an optional fallback to a specific proxy.
    RemoteProxyClientFromBootstrap {
        /// The bootstrap server from which to fetch the proxy_list
        bootstrap_url: Url2,

        /// The optional fallback specific proxy server
        fallback_proxy_url: Option<Url2>,
    },

    /// We want to be a proxy server for others.
    /// (We can also deny all proxy requests for something in-between.)
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
