use crate::*;

pub use kitsune_p2p_types::tls::TlsConfig;

/// Callback function signature for proxy accept/deny.
pub type AcceptProxyCallbackFn =
    Arc<dyn Fn(CertDigest) -> MustBoxFuture<'static, bool> + 'static + Send + Sync>;

/// Callback type for proxy accept/deny.
#[derive(Clone, Deref, AsRef)]
pub struct AcceptProxyCallback(pub AcceptProxyCallbackFn);

impl AcceptProxyCallback {
    /// Callback that blanket denies all proxy requests.
    pub fn reject_all() -> Self {
        Self(Arc::new(|_| async { false }.boxed().into()))
    }

    /// Callback that blanket accepts all proxy requests.
    pub fn accept_all() -> Self {
        Self(Arc::new(|_| async { true }.boxed().into()))
    }
}

/// Configuration for proxy binding.
pub enum ProxyConfig {
    /// We want to be hosted at a remote proxy location.
    RemoteProxyClient {
        /// The Tls config for this proxy endpoint.
        tls: TlsConfig,

        /// The remote proxy url to be hosted at.
        proxy_url: ProxyUrl,
    },

    /// We want to be a proxy server for others.
    /// (We can also deny all proxy requests for something in-between).
    LocalProxyServer {
        /// The Tls config for this proxy endpoint.
        tls: TlsConfig,

        /// Return true if we should take on proxying for the
        /// requesting client.
        accept_proxy_cb: AcceptProxyCallback,
    },
}

impl ProxyConfig {
    /// We want to be hosted at a remote proxy location.
    pub fn remote_proxy_client(tls: TlsConfig, proxy_url: ProxyUrl) -> Arc<Self> {
        Arc::new(Self::RemoteProxyClient { tls, proxy_url })
    }

    /// We want to be a proxy server for others.
    /// (We can also deny all proxy requests for something in-between).
    pub fn local_proxy_server(tls: TlsConfig, accept_proxy_cb: AcceptProxyCallback) -> Arc<Self> {
        Arc::new(Self::LocalProxyServer {
            tls,
            accept_proxy_cb,
        })
    }
}

/// Tls ALPN identifier for kitsune proxy handshaking
const ALPN_KITSUNE_PROXY_0: &[u8] = b"kitsune-proxy/0";

/// Helper to generate rustls configs given a TlsConfig reference.
#[allow(dead_code)]
pub(crate) fn gen_tls_configs(
    tls: &TlsConfig,
    tuning_params: Arc<kitsune_p2p_types::config::KitsuneP2pTuningParams>,
) -> TransportResult<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>)> {
    kitsune_p2p_types::tls::gen_tls_configs(ALPN_KITSUNE_PROXY_0, tls, tuning_params)
        .map_err(TransportError::other)
}
