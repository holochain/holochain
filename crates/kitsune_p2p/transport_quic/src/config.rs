use crate::*;

/// Configuration struct for spawn_transport_listener_quic()
#[non_exhaustive]
#[derive(Default)]
pub struct ConfigListenerQuic {
    /// To which network interface / port should we bind?
    /// Default: "kitsune-quic://0.0.0.0:0".
    pub bind_to: Option<Url2>,

    /// If you have port-forwarding set up,
    /// or wish to apply a vanity domain name,
    /// you may need to override the local NIC ip.
    /// Default: None = use NIC ip.
    pub override_host: Option<String>,

    /// If you have port-forwarding set up,
    /// you may need to override the local NIC port.
    /// Default: None = use NIC port.
    pub override_port: Option<u16>,

    /// Tls config
    /// Default: None = ephemeral.
    pub tls: Option<(
        lair_keystore_api_0_0::actor::Cert,
        lair_keystore_api_0_0::actor::CertPrivKey,
    )>,
}

impl ConfigListenerQuic {
    /// Set 'bind_to' builder pattern.
    pub fn set_bind_to(mut self, bind_to: Option<Url2>) -> Self {
        self.bind_to = bind_to;
        self
    }

    /// Set 'override_host' builder pattern.
    pub fn set_override_host<S: Into<String>>(mut self, override_host: Option<S>) -> Self {
        self.override_host = override_host.map(|s| s.into());
        self
    }

    /// Set 'override_port' builder pattern.
    pub fn set_override_port(mut self, override_port: Option<u16>) -> Self {
        self.override_port = override_port;
        self
    }

    /// Set 'tls' builder pattern.
    pub fn set_tls(
        mut self,
        tls: Option<(
            lair_keystore_api_0_0::actor::Cert,
            lair_keystore_api_0_0::actor::CertPrivKey,
        )>,
    ) -> Self {
        self.tls = tls;
        self
    }
}
