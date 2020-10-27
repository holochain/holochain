//! Utilities to make kitsune testing a little more sane.

use crate::*;

/// initialize tracing
pub fn init_tracing() {
    let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    );
}

/// test_proxy_config_mem
pub fn test_proxy_config_mem() -> KitsuneP2pConfig {
    let mut config = KitsuneP2pConfig::default();
    config.transport_pool.push(TransportConfig::Proxy {
        sub_transport: Box::new(TransportConfig::Mem {}),
        proxy_config: ProxyConfig::LocalProxyServer {
            proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
        },
    });
    config
}

/// test_proxy_config_quic
pub fn test_proxy_config_quic() -> KitsuneP2pConfig {
    let mut config = KitsuneP2pConfig::default();
    config.transport_pool.push(TransportConfig::Proxy {
        sub_transport: Box::new(TransportConfig::Quic {
            bind_to: Some(url2::url2!("kitsune-quic://0.0.0.0:0")),
            override_host: None,
            override_port: None,
        }),
        proxy_config: ProxyConfig::LocalProxyServer {
            proxy_accept_config: Some(ProxyAcceptConfig::RejectAll),
        },
    });
    config
}
