use crate::*;

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

/// Tls Configuration for proxy.
#[derive(Clone)]
pub struct TlsConfig {
    /// Cert
    pub cert: Cert,

    /// Cert Priv Key
    pub cert_priv_key: CertPrivKey,

    /// Cert Digest
    pub cert_digest: CertDigest,
}

impl TlsConfig {
    /// Create a new ephemeral tls certificate that will not be persisted.
    pub async fn new_ephemeral() -> TransportResult<Self> {
        let mut options = lair_keystore_api::actor::TlsCertOptions::default();
        options.alg = lair_keystore_api::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
        let cert = lair_keystore_api::internal::tls::tls_cert_self_signed_new_from_entropy(options)
            .await
            .map_err(TransportError::other)?;
        Ok(Self {
            cert: cert.cert_der,
            cert_priv_key: cert.priv_key_der,
            cert_digest: cert.cert_digest,
        })
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
