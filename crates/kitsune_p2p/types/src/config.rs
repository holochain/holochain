//! Kitsune Config Tuning Params
#![allow(missing_docs)]

use crate::tx2::tx2_adapter::AdapterFactory;
use crate::tx2::tx2_utils::TxUrl;
use crate::KitsuneResult;
use url2::Url2;

/// Fifteen minutes
pub const RECENT_THRESHOLD_DEFAULT: std::time::Duration = std::time::Duration::from_secs(60 * 15);

/// Wrapper for the actual KitsuneP2pTuningParams struct
/// so the widely used type def can be an Arc<>
pub mod tuning_params_struct {
    use ghost_actor::dependencies::tracing;
    use kitsune_p2p_dht::{
        prelude::{ArqClamping, LocalStorageConfig},
        ArqStrat,
    };
    use kitsune_p2p_dht_arc::DEFAULT_MIN_PEERS;
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
        /// to this count megabits per second. [Default: 100.0]
        gossip_outbound_target_mbps: f64 = 100.0,

        /// The gossip loop will attempt to rate-limit input
        /// to this count megabits per second. [Default: 100.0]
        gossip_inbound_target_mbps: f64 = 100.0,

        /// The gossip loop will attempt to rate-limit outbound
        /// traffic for the historic loop (if there is one)
        /// to this count megabits per second. [Default: 100.0]
        gossip_historic_outbound_target_mbps: f64 = 100.0,

        /// The gossip loop will attempt to rate-limit inbound
        /// traffic for the historic loop (if there is one)
        /// to this count megabits per second. [Default: 100.0]
        gossip_historic_inbound_target_mbps: f64 = 100.0,

        /// The gossip loop accomodates this amount of excess capacity
        /// before enacting the target rate limit, expressed as a ratio
        /// of the target rate limit. For instance, if the historic
        /// outbound target is 10mbps, a burst ratio of 50 will allow
        /// an extra 500mb of outbound traffic before the target rate
        /// limiting kicks in (and this extra capacity will take 50
        /// seconds to "refill"). [Default: 100.0]
        gossip_burst_ratio: f64 = 100.0,

        /// How long should we hold off talking to a peer
        /// we've previously spoken successfully to.
        /// [Default: 1 minute]
        gossip_peer_on_success_next_gossip_delay_ms: u32 = 1000 * 60,

        /// How long should we hold off talking to a peer
        /// we've previously gotten errors speaking to.
        /// [Default: 5 minute]
        gossip_peer_on_error_next_gossip_delay_ms: u32 = 1000 * 60 * 5,

        /// How often should we update and publish our agent info?
        /// [Default: 5 minutes]
        gossip_agent_info_update_interval_ms: u32 = 1000 * 60 * 5,

        /// The timeout for a gossip round if there is no contact.
        /// [Default: 1 minute]
        gossip_round_timeout_ms: u64 = 1000 * 60,

        /// The target redundancy is the number of peers we expect to hold any
        /// given Op.
        gossip_redundancy_target: f64 = DEFAULT_MIN_PEERS as f64,

        /// The max number of bytes of data to send in a single message.
        ///
        /// This setting was more relevant when entire Ops were being gossiped,
        /// but now that only hashes are gossiped, it would take a lot of hashes
        /// to reach this limit (1MB = approx 277k hashes).
        ///
        /// Payloads larger than this are split into multiple batches
        /// when possible.
        gossip_max_batch_size: u32 = 1_000_000,

        /// Should gossip dynamically resize storage arcs?
        gossip_dynamic_arcs: bool = true,

        /// By default, Holochain adjusts the gossip_arc to match the
        /// the current network conditions for the given DNA.
        /// If unsure, please keep this setting at the default "none",
        /// meaning no arc clamping. Setting options are:
        /// - "none" - Keep the default auto-adjust behavior.
        /// - "empty" - Makes you a freeloader, contributing nothing
        ///   to the network. Please don't choose this option without
        ///   a good reason, such as being on a bandwidth constrained
        ///   mobile device!
        /// - "full" - Indicates that you commit to serve and hold all
        ///   all data from all agents, and be a potential target for all
        ///   get requests. This could be a significant investment of
        ///   bandwidth. Don't take this responsibility lightly.
        gossip_arc_clamping: String = "none".to_string(),

        /// Default timeout for rpc single. [Default: 60s]
        default_rpc_single_timeout_ms: u32 = 1000 * 60,

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
        /// [Default: 60 seconds]
        tx2_quic_max_idle_timeout_ms: u32 = 1000 * 60,

        /// tx2 pool max connection count
        /// [Default: 4096]
        tx2_pool_max_connection_count: usize = 4096,

        /// tx2 channel count per connection
        /// [Default: 2]
        tx2_channel_count_per_connection: usize = 2,

        /// tx2 timeout used for passive background operations
        /// like reads / responds.
        /// [Default: 60 seconds]
        tx2_implicit_timeout_ms: u32 = 1000 * 60,

        /// tx2 initial connect retry delay
        /// (note, this delay is currenty exponentially backed off--
        /// multiplied by 2x on every loop)
        /// [Default: 200 ms]
        tx2_initial_connect_retry_delay_ms: usize = 200,

        /// Maximum count of open connections.
        /// [Default: 4096]
        tx5_connection_count_max: u32 = 4096,

        /// Max backend send buffer bytes (per connection).
        /// [Default: 64 KiB]
        tx5_send_buffer_bytes_max: u32 = 64 * 1024,

        /// Max backend recv buffer bytes (per connection).
        /// [Default: 64 KiB]
        tx5_recv_buffer_bytes_max: u32 = 64 * 1024,

        /// Maximum receive message reconstruction bytes in memory
        /// (accross entire endpoint).
        /// [Default: 512 MiB]
        tx5_incoming_message_bytes_max: u32 = 512 * 1024 * 1024,

        /// Maximum size of an individual message.
        /// [Default: 16 MiB]
        tx5_message_size_max: u32 = 16 * 1024 * 1024,

        /// Internal event channel size.
        /// [Default: 1024]
        tx5_internal_event_channel_size: u32 = 1024,

        /// Default timeout for network operations.
        /// [Default: 60]
        tx5_timeout_s: u32 = 60,

        /// Starting backoff duration for retries.
        /// [Default: 60]
        tx5_backoff_start_s: u32 = 5,

        /// Max backoff duration for retries.
        /// [Default: 60]
        tx5_backoff_max_s: u32 = 60,

        /// Tx5 ban time in seconds.
        tx5_ban_time_s: u32 = 10,

        /// Tx5 min ephemeral port
        tx5_min_ephemeral_udp_port: u16 = 1,

        /// Tx5 max ephemeral port
        tx5_max_ephemeral_udp_port: u16 = 65535,

        /// if you would like to be able to use an external tool
        /// to debug the QUIC messages sent and received by kitsune
        /// you'll need the decryption keys.
        /// The default of `"no_keylog"` is secure and will not write any keys
        /// Setting this to `"env_keylog"` will write to a keylog specified
        /// by the `SSLKEYLOGFILE` environment variable, or do nothing if
        /// it is not set, or is not writable.
        danger_tls_keylog: String = "no_keylog".to_string(),

        /// Set the cutoff time when gossip switches over from recent
        /// to historical gossip.
        ///
        /// This is dangerous to change, because gossip may not be
        /// possible with nodes using a different setting for this threshold.
        /// Do not change this except in testing environments.
        /// [Default: 15 minutes]
        danger_gossip_recent_threshold_secs: u64 = super::RECENT_THRESHOLD_DEFAULT.as_secs(),

        /// Don't publish ops, only rely on gossip. Useful for testing the efficacy of gossip.
        disable_publish: bool = false,

        /// Disable recent gossip. Useful for testing Historical gossip in isolation.
        /// Note that this also disables agent gossip!
        disable_recent_gossip: bool = false,

        /// Disable historical gossip. Useful for testing Recent gossip in isolation.
        disable_historical_gossip: bool = false,

        /// Control the backoff multiplier for the time delay between checking in with the bootstrap server.
        /// The default value of `2` causes the delay to grow quickly up to the max time of 1 hour.
        /// For testing consider using `1` to prevent the delay from growing.
        bootstrap_check_delay_backoff_multiplier: u32 = 2,

        /// Set the bootstrap fetch maximum backoff time.
        /// The default value is 60 * 5 s = five minutes.
        /// The minimum value is 60 s = one minute.
        bootstrap_max_delay_s: u32 = 60 * 5,
    }

    impl KitsuneP2pTuningParams {
        /// Generate a KitsuneTimeout instance
        /// based on the tuning parameter tx2_implicit_timeout_ms
        pub fn implicit_timeout(&self) -> crate::KitsuneTimeout {
            crate::KitsuneTimeout::from_millis(self.tx2_implicit_timeout_ms as u64)
        }

        /// Get the gossip recent threshold param as a proper Duration
        pub fn danger_gossip_recent_threshold(&self) -> std::time::Duration {
            std::time::Duration::from_secs(self.danger_gossip_recent_threshold_secs)
        }

        /// get the tx5_ban_time_s param as a Duration.
        pub fn tx5_ban_time(&self) -> std::time::Duration {
            std::time::Duration::from_secs(self.tx5_ban_time_s as u64)
        }

        /// returns true if we should initialize a tls keylog
        /// based on the `SSLKEYLOGFILE` environment variable
        pub fn use_env_tls_keylog(&self) -> bool {
            self.danger_tls_keylog == "env_keylog"
        }

        /// The timeout for a gossip round if there is no contact.
        pub fn gossip_round_timeout(&self) -> std::time::Duration {
            std::time::Duration::from_millis(self.gossip_round_timeout_ms)
        }

        /// Parse the gossip_arc_clamping string as a proper type
        pub fn arc_clamping(&self) -> Option<ArqClamping> {
            match self.gossip_arc_clamping.to_lowercase().as_str() {
                "none" => None,
                "empty" => Some(ArqClamping::Empty),
                "full" => Some(ArqClamping::Full),
                other => panic!("Invalid kitsune tuning param: arc_clamping = '{}'", other),
            }
        }

        /// Create a standard ArqStrat from the tuning params
        pub fn to_arq_strat(&self) -> ArqStrat {
            let local_storage = LocalStorageConfig {
                arc_clamping: self.arc_clamping(),
            };
            ArqStrat::standard(local_storage, self.gossip_redundancy_target)
        }
    }
}

/// We don't want to clone these tuning params over-and-over.
/// They should normally be passed around as an Arc.
pub type KitsuneP2pTuningParams = std::sync::Arc<tuning_params_struct::KitsuneP2pTuningParams>;

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

    /// All tracing logs from kitsune tasks will be instrumented to contain this string,
    /// so that logs from multiple instances in the same process can be disambiguated.
    #[serde(default)]
    pub tracing_scope: Option<String>,
}

impl Default for KitsuneP2pConfig {
    fn default() -> Self {
        Self {
            transport_pool: Vec::new(),
            bootstrap_service: None,
            tuning_params: KitsuneP2pTuningParams::default(),
            network_type: NetworkType::QuicBootstrap,
            tracing_scope: None,
        }
    }
}

#[allow(dead_code)]
fn cnv_bind_to(bind_to: &Option<url2::Url2>) -> TxUrl {
    match bind_to {
        Some(bind_to) => bind_to.clone().into(),
        None => TxUrl::from_str_panicking("kitsune-quic://0.0.0.0:0"),
    }
}

impl KitsuneP2pConfig {
    #[allow(dead_code)] // because of feature flipping
    pub fn is_tx2(&self) -> bool {
        #[cfg(feature = "tx2")]
        {
            #[cfg(feature = "tx5")]
            {
                if let Some(t) = self.transport_pool.first() {
                    !matches!(t, TransportConfig::WebRTC { .. })
                } else {
                    true
                }
            }
            #[cfg(not(feature = "tx5"))]
            {
                true
            }
        }
        #[cfg(not(feature = "tx2"))]
        {
            false
        }
    }

    /// This config is making use of tx5 transport
    #[allow(dead_code)] // because of feature flipping
    pub fn is_tx5(&self) -> bool {
        #[cfg(feature = "tx5")]
        {
            if let Some(t) = self.transport_pool.first() {
                return matches!(t, TransportConfig::WebRTC { .. });
            }
        }
        false
    }

    /// `tx2` is currently designed to use exactly one proxy wrapped transport,
    /// so convert a bunch of the options from the previous transport
    /// paradigm into that pattern.
    #[cfg(feature = "tx2")]
    pub fn to_tx2(&self) -> KitsuneResult<KitsuneP2pTx2Config> {
        use KitsuneP2pTx2ProxyConfig::*;
        match self.transport_pool.first() {
            Some(TransportConfig::Mock { mock_network }) => Ok(KitsuneP2pTx2Config {
                backend: KitsuneP2pTx2Backend::Mock {
                    mock_network: mock_network.0.clone(),
                },
                use_proxy: NoProxy,
            }),
            #[cfg(feature = "tx5")]
            Some(TransportConfig::WebRTC { .. }) => {
                Err("Cannot convert tx5 config into tx2".into())
            }
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
    #[cfg(feature = "tx2")]
    Mem {},
    /// A mock network for testing
    #[cfg(feature = "tx2")]
    #[serde(skip)]
    Mock {
        /// The adaptor for mocking the network
        mock_network: AdapterFactoryMock,
    },
    /// Configure to use Tx5 WebRTC for kitsune networking.
    #[cfg(feature = "tx5")]
    #[serde(rename = "webrtc", alias = "web_r_t_c", alias = "web_rtc")]
    WebRTC {
        /// The url of the signal server to connect to for addressability.
        signal_url: String,
    },
}

pub enum KitsuneP2pTx2Backend {
    #[allow(dead_code)]
    #[cfg(feature = "tx2")]
    Mem,
    //#[cfg(feature = "tx2")]
    //Quic { bind_to: TxUrl },
    #[allow(dead_code)]
    #[cfg(feature = "tx2")]
    Mock { mock_network: AdapterFactory },
}

#[cfg(feature = "tx2")]
pub enum KitsuneP2pTx2ProxyConfig {
    NoProxy,
    #[allow(dead_code)]
    Specific(TxUrl),
    #[allow(dead_code)]
    Bootstrap {
        #[allow(dead_code)]
        bootstrap_url: TxUrl,
        fallback_proxy_url: Option<TxUrl>,
    },
}

#[cfg(feature = "tx2")]
pub struct KitsuneP2pTx2Config {
    pub backend: KitsuneP2pTx2Backend,
    pub use_proxy: KitsuneP2pTx2ProxyConfig,
}

/// Proxy configuration options
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg(feature = "tx2")]
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
#[cfg(feature = "tx2")]
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
    // MAYBE: Remove the "Quic" from this?
    QuicBootstrap,
    /// Via MDNS to the LAN
    // MAYBE: Remove the "Quic" from this?
    QuicMdns,
}

#[cfg(feature = "tx2")]
#[derive(Clone)]
/// A simple wrapper around the [`AdapterFactory`]
/// to allow implementing Debug and PartialEq.
pub struct AdapterFactoryMock(pub AdapterFactory);

#[cfg(feature = "tx2")]
impl std::fmt::Debug for AdapterFactoryMock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AdapterFactoryMock").finish()
    }
}

#[cfg(feature = "tx2")]
impl std::cmp::PartialEq for AdapterFactoryMock {
    fn eq(&self, _: &Self) -> bool {
        unimplemented!()
    }
}

#[cfg(feature = "tx2")]
impl From<AdapterFactory> for AdapterFactoryMock {
    fn from(adaptor_factory: AdapterFactory) -> Self {
        Self(adaptor_factory)
    }
}
