//! Kitsune Config Params

use crate::tx2::tx2_adapter::AdapterFactory;
use url2::Url2;
use crate::KitsuneResult;
use crate::tx2::tx2_utils::TxUrl;

/// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default production bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEFAULT: &str = "https://bootstrap-staging.holo.host";

/// TODO - FIXME - holochain bootstrap should not be encoded in kitsune
/// The default development bootstrap service url.
pub const BOOTSTRAP_SERVICE_DEV: &str = "https://bootstrap-dev.holohost.workers.dev";

#[allow(missing_docs)]
pub enum KitsuneP2pTx2Backend {
    Mem,
    Quic { bind_to: TxUrl },
    Mock { mock_network: AdapterFactory },
}

#[allow(missing_docs)]
pub struct KitsuneP2pTx2Config {
    pub backend: KitsuneP2pTx2Backend,
    pub use_proxy: Option<TxUrl>,
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
    /// tx2 is currently designed to use exactly one proxy wrapped transport
    /// so, convert a bunch of the options from the previous transport
    /// paradigm into that pattern.
    pub fn to_tx2(&self) -> KitsuneResult<KitsuneP2pTx2Config> {
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
                    ProxyConfig::RemoteProxyClient { proxy_url } => Some(proxy_url.clone().into()),
                    ProxyConfig::LocalProxyServer { .. } => None,
                };
                Ok(KitsuneP2pTx2Config { backend, use_proxy })
            }
            Some(TransportConfig::Quic { bind_to, .. }) => {
                let bind_to = cnv_bind_to(bind_to);
                Ok(KitsuneP2pTx2Config {
                    backend: KitsuneP2pTx2Backend::Quic { bind_to },
                    use_proxy: None,
                })
            }
            Some(TransportConfig::Mock { mock_network }) => Ok(KitsuneP2pTx2Config {
                backend: KitsuneP2pTx2Backend::Mock {
                    mock_network: mock_network.0.clone(),
                },
                use_proxy: None,
            }),
            None | Some(TransportConfig::Mem {}) => Ok(KitsuneP2pTx2Config {
                backend: KitsuneP2pTx2Backend::Mem,
                use_proxy: None,
            }),
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
    #[serde(skip)]
    /// A mock network for testing.
    Mock {
        /// The adaptor for mocking the network.
        mock_network: AdapterFactoryMock,
    },
}

#[derive(Clone)]
/// A simple wrapper around the [`AdaptorFactory`] to allow implementing
/// Debug and PartialEq.
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


/// How long kitsune should wait before timing out when joining the network.
pub const JOIN_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Wrapper for the actual KitsuneP2pTuningParams struct
/// so the widely used type def can be an Arc<>
pub mod tuning_params_struct {
    use ghost_actor::dependencies::tracing;
    use std::collections::HashMap;

    macro_rules! mk_tune {
        ($($(#[doc = $doc:expr])* $i:ident: $t:ty = $d:expr,)*) => {
            /// Network tuning parameters.
            /// This is serialized carefully so all the values can be represented
            /// as strings in YAML - and we will be able to proceed with a printed
            /// warning for tuning params that are removed, but still specified in
            /// configs.
            #[non_exhaustive]
            #[derive(Clone, Debug, PartialEq)]
            pub struct KitsuneP2pTuningParams {
                $(
                    $(#[doc = $doc])*
                    pub $i: $t,
                )*
            }

            impl Default for KitsuneP2pTuningParams {
                fn default() -> Self {
                    Self {
                        $(
                            $i: $d,
                        )*
                    }
                }
            }

            impl serde::Serialize for KitsuneP2pTuningParams {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    use serde::ser::SerializeMap;
                    let mut m = serializer.serialize_map(None)?;
                    $(
                        m.serialize_entry(
                            stringify!($i),
                            &format!("{}", &self.$i),
                        )?;
                    )*
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
                            $(
                                stringify!($i) => match v.parse::<$t>() {
                                    Ok(v) => out.$i = v,
                                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                                },
                            )*
                            _ => tracing::warn!("INVALID TUNING PARAM: '{}'", k),
                        }
                    }
                    Ok(out)
                }
            }
        };
    }

    mk_tune! {
        /// Gossip strategy to use. [Default: "sharded-gossip"]
        gossip_strategy: String = "sharded-gossip".to_string(),

        /// Delay between gossip loop iteration. [Default: 1s]
        gossip_loop_iteration_delay_ms: u32 = 1000,

        /// The gossip loop will attempt to rate-limit output
        /// to this count mega bits per second. [Default: 0.5]
        gossip_outbound_target_mbps: f64 = 0.5,

        /// The gossip loop will attempt to rate-limit input
        /// to this count mega bits per second. [Default: 0.5]
        gossip_inbound_target_mbps: f64 = 0.5,

        /// The gossip loop will attempt to rate-limit outbound
        /// traffic for the historic loop (if there is one)
        /// to this count mega bits per second. [Default: 0.1]
        gossip_historic_outbound_target_mbps: f64 = 0.1,

        /// The gossip loop will attempt to rate-limit inbound
        /// traffic for the historic loop (if there is one)
        /// to this count mega bits per second. [Default: 0.1]
        gossip_historic_inbound_target_mbps: f64 = 0.1,

        /// How long should we hold off talking to a peer
        /// we've previously spoken successfully to.
        /// [Default: 1 minute]
        gossip_peer_on_success_next_gossip_delay_ms: u32 = 1000 * 60,

        /// How long should we hold off talking to a peer
        /// we've previously gotten errors speaking to.
        /// [Default: 5 minute]
        gossip_peer_on_error_next_gossip_delay_ms: u32 = 1000 * 60 * 5,

        /// How frequently we should locally sync when there is
        /// no new data. Agents arc can change so this shouldn't
        /// be too long. [Default: 1 minutes]
        gossip_local_sync_delay_ms: u32 = 1000 * 60,

        /// Should gossip dynamically resize storage arcs?
        gossip_dynamic_arcs: bool = false,

        /// Allow only the first agent to join the space to
        /// have a sized storage arc. [Default: false]
        /// This is an experimental feature that sets the first
        /// agent to join as the full arc and all other later
        /// agents to empty.
        /// It should not be used in production unless you understand
        /// what you are doing.
        gossip_single_storage_arc_per_space: bool = false,

        /// Default timeout for rpc single. [Default: 30s]
        default_rpc_single_timeout_ms: u32 = 1000 * 30,

        /// Default agent count for rpc multi. [Default: 3]
        default_rpc_multi_remote_agent_count: u8 = 3,

        /// Default remote request grace ms. [Default: 3s]
        /// If we already have results from other sources,
        /// but made any additional outgoing remote requests,
        /// we'll wait at least this long for additional responses.
        default_rpc_multi_remote_request_grace_ms: u64 = 1000 * 3,

        /// Default agent expires after milliseconds. [Default: 20 minutes]
        agent_info_expires_after_ms: u32 = 1000 * 60 * 20,

        /// Tls in-memory session storage capacity. [Default: 512]
        tls_in_mem_session_storage: u32 = 512,

        /// How often should NAT nodes refresh their proxy contract?
        /// [Default: 2 minutes]
        proxy_keepalive_ms: u32 = 1000 * 60 * 2,

        /// How often should proxy nodes prune their ProxyTo list?
        /// Note - to function this should be > proxy_keepalive_ms.
        /// [Default: 5 minutes]
        proxy_to_expire_ms: u32 = 1000 * 60 * 5,

        /// Mainly used as the for_each_concurrent limit,
        /// this restricts the number of active polled futures
        /// on a single thread.
        /// [Default: 4096]
        concurrent_limit_per_thread: usize = 4096,

        /// tx2 quic max_idle_timeout
        /// [Default: 30 seconds]
        tx2_quic_max_idle_timeout_ms: u32 = 1000 * 30,

        /// tx2 pool max connection count
        /// [Default: 4096]
        tx2_pool_max_connection_count: usize = 4096,

        /// tx2 channel count per connection
        /// [Default: 16]
        tx2_channel_count_per_connection: usize = 16,

        /// tx2 timeout used for passive background operations
        /// like reads / responds.
        /// [Default: 30 seconds]
        tx2_implicit_timeout_ms: u32 = 1000 * 30,

        /// tx2 initial connect retry delay
        /// (note, this delay is currenty exponentially backed off--
        /// multiplied by 2x on every loop)
        /// [Default: 200 ms]
        tx2_initial_connect_retry_delay_ms: usize = 200,
    }

    impl KitsuneP2pTuningParams {
        /// Generate a KitsuneTimeout instance
        /// based on the tuning parameter tx2_implicit_timeout_ms
        pub fn implicit_timeout(&self) -> crate::KitsuneTimeout {
            crate::KitsuneTimeout::from_millis(self.tx2_implicit_timeout_ms as u64)
        }
    }
}

/// We don't want to clone these tuning params over-and-over.
/// They should normally be passed around as an Arc.
pub type KitsuneP2pTuningParams = std::sync::Arc<tuning_params_struct::KitsuneP2pTuningParams>;
