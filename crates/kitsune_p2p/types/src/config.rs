//! Kitsune Config Tuning Params

/// How long kitsune should wait before timing out when joining the network.
pub const JOIN_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

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

        /// Tx5 max pending send byte count limit.
        /// [Default: 16 MiB]
        tx5_max_send_bytes: u32 = 16 * 1024 * 1024,

        /// Tx5 max pending recv byte count limit.
        /// [Default: 16 MiB]
        tx5_max_recv_bytes: u32 = 16 * 1024 * 1024,

        /// Tx5 max concurrent connection limit.
        /// [Default: 255]
        tx5_max_conn_count: u32 = 255,

        /// Tx5 max init (connect) time for a connection in seconds.
        /// [Default: 60]
        tx5_max_conn_init_s: u32 = 60,

        /// Tx5 ban time in seconds.
        tx5_ban_time_s: u32 = 10,

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

        /// Get the tx5_max_conn_init_s param as a Duration.
        pub fn tx5_max_conn_init(&self) -> std::time::Duration {
            std::time::Duration::from_secs(self.tx5_max_conn_init_s as u64)
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
            ArqStrat::standard(local_storage)
        }
    }
}

/// We don't want to clone these tuning params over-and-over.
/// They should normally be passed around as an Arc.
pub type KitsuneP2pTuningParams = std::sync::Arc<tuning_params_struct::KitsuneP2pTuningParams>;
