//! Kitsune Config Tuning Params

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
    /// Delay between gossip loop iteration. [Default: 10ms]
    gossip_loop_iteration_delay_ms: u32 = 10,

    /// Default agent count for remote notify. [Default: 5]
    default_notify_remote_agent_count: u32 = 5,

    /// Default timeout for remote notify. [Default: 1000ms]
    default_notify_timeout_ms: u32 = 1000,

    /// Default timeout for rpc single. [Default: 2000]
    default_rpc_single_timeout_ms: u32 = 2000,

    /// Default agent count for rpc multi. [Default: 2]
    default_rpc_multi_remote_agent_count: u32 = 2,

    /// Default timeout for rpc multi. [Default: 2000]
    default_rpc_multi_timeout_ms: u32 = 2000,

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
}
