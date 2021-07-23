//! Kitsune Config Tuning Params

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
        /// Gossip strategy to use. [Default: simple-bloom]
        gossip_strategy: String = "simple-bloom".to_string(),

        /// Delay between gossip loop iteration. [Default: 1s]
        gossip_loop_iteration_delay_ms: u32 = 1000,

        /// The gossip loop will attempt to rate-limit output
        /// to this count mega bits per second. [Default: 0.5]
        gossip_output_target_mbps: f64 = 0.5,

        /// The gossip loop will attempt to rate-limit input
        /// to this count mega bits per second. [Default: 0.5]
        gossip_inbound_target_mbps: f64 = 0.5,

        /// How long should we hold off talking to a peer
        /// we've previously spoken successfully to.
        /// [Default: 1 minute]
        gossip_peer_on_success_next_gossip_delay_ms: u32 = 1000 * 60,

        /// How long should we hold off talking to a peer
        /// we've previously gotten errors speaking to.
        /// [Default: 5 minute]
        gossip_peer_on_error_next_gossip_delay_ms: u32 = 1000 * 60 * 5,

        /// Default timeout for rpc single. [Default: 30s]
        default_rpc_single_timeout_ms: u32 = 1000 * 30,

        /// Default agent count for rpc multi. [Default: 2]
        default_rpc_multi_remote_agent_count: u32 = 2,

        /// Default timeout for rpc multi. [Default: 30s]
        default_rpc_multi_timeout_ms: u32 = 1000 * 30,

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
