//! Kitsune Config Tuning Params

use ghost_actor::dependencies::tracing;
use std::collections::HashMap;

/// Network tuning parameters.
/// This is serialized carefully so all the values can be represented
/// as strings in YAML - and we will be able to proceed with a printed
/// warning for tuning params that are removed, but still specified in
/// configs.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct KitsuneP2pTuningParams {
    /// Delay between gossip loop iteration. [Default: 10ms]
    pub gossip_loop_iteration_delay_ms: u32,

    /// Default agent count for remote notify. [Default: 5]
    pub default_notify_remote_agent_count: u32,

    /// Default timeout for remote notify. [Default: 1000ms]
    pub default_notify_timeout_ms: u32,

    /// Default timeout for rpc single. [Default: 2000]
    pub default_rpc_single_timeout_ms: u32,

    /// Default agent count for rpc multi. [Default: 2]
    pub default_rpc_multi_remote_agent_count: u32,

    /// Default timeout for rpc multi. [Default: 2000]
    pub default_rpc_multi_timeout_ms: u32,

    /// Default agent expires after milliseconds. [Default: 20 minutes]
    pub agent_info_expires_after_ms: u32,

    /// Tls in-memory session storage capacity. [Default: 512]
    pub tls_in_mem_session_storage: u32,

    /// How often should NAT nodes refresh their proxy contract?
    /// [Default: 2 minutes]
    pub proxy_keepalive_ms: u32,

    /// How often should proxy nodes prune their ProxyTo list?
    /// Note - to function this should be > proxy_keepalive_ms.
    /// [Default: 5 minutes]
    pub proxy_to_expire_ms: u32,
}

/// How long kitsune should wait before timing out when joining the network.
pub const JOIN_NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

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
            tls_in_mem_session_storage: 512,
            proxy_keepalive_ms: 1000 * 60 * 2, // 2 minutes
            proxy_to_expire_ms: 1000 * 60 * 5, // 5 minutes
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
        m.serialize_entry(
            "tls_in_mem_session_storage",
            &format!("{}", self.tls_in_mem_session_storage),
        )?;
        m.serialize_entry(
            "proxy_keepalive_ms",
            &format!("{}", self.proxy_keepalive_ms),
        )?;
        m.serialize_entry(
            "proxy_to_expire_ms",
            &format!("{}", self.proxy_to_expire_ms),
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
                "tls_in_mem_session_storage" => match v.parse::<u32>() {
                    Ok(v) => out.tls_in_mem_session_storage = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "proxy_keepalive_ms" => match v.parse::<u32>() {
                    Ok(v) => out.proxy_keepalive_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                "proxy_to_expire_ms" => match v.parse::<u32>() {
                    Ok(v) => out.proxy_to_expire_ms = v,
                    Err(e) => tracing::warn!("failed to parse {}: {}", k, e),
                },
                _ => tracing::warn!("INVALID TUNING PARAM: '{}'", k),
            }
        }
        Ok(out)
    }
}
