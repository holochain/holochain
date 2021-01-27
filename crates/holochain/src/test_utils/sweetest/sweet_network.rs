use kitsune_p2p::KitsuneP2pConfig;

/// Helper for constructing common kitsune networks
pub struct SweetNetwork;

impl SweetNetwork {
    /// Get a remote kitsune proxy address from the
    /// env var `KIT_PROXY` if it's set.
    pub fn env_var_proxy() -> Option<KitsuneP2pConfig> {
        std::env::var_os("KIT_PROXY").map(|proxy_addr| {
            let mut network = KitsuneP2pConfig::default();
            let transport = kitsune_p2p::TransportConfig::Quic {
                bind_to: None,
                override_port: None,
                override_host: None,
            };
            let proxy_config = holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient {
                proxy_url: url2::url2!("{}", proxy_addr.into_string().unwrap()),
            };
            network.transport_pool = vec![kitsune_p2p::TransportConfig::Proxy {
                sub_transport: transport.into(),
                proxy_config,
            }];
            network
        })
    }

    /// Local quic proxy network
    pub fn local_quic() -> KitsuneP2pConfig {
        let mut network = KitsuneP2pConfig::default();
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        network
    }
}
