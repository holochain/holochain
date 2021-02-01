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

/// Tls ALPN identifier for kitsune proxy handshaking
const ALPN_KITSUNE_PROXY_0: &[u8] = b"kitsune-proxy/0";

/// Allow only these cipher suites for kitsune proxy Tls.
static CIPHER_SUITES: &[&rustls::SupportedCipherSuite] = &[
    &rustls::ciphersuite::TLS13_CHACHA20_POLY1305_SHA256,
    &rustls::ciphersuite::TLS13_AES_256_GCM_SHA384,
];

/// Helper to generate rustls configs given a TlsConfig reference.
#[allow(dead_code)]
pub(crate) fn gen_tls_configs(
    tls: &TlsConfig,
    tuning_params: Arc<kitsune_p2p_types::config::KitsuneP2pTuningParams>,
) -> TransportResult<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>)> {
    let cert = rustls::Certificate(tls.cert.0.to_vec());
    let cert_priv_key = rustls::PrivateKey(tls.cert_priv_key.0.to_vec());

    let root_cert = rustls::Certificate(lair_keystore_api::internal::tls::WK_CA_CERT_DER.to_vec());
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(&root_cert).unwrap();

    let mut tls_server_config = rustls::ServerConfig::with_ciphersuites(
        rustls::AllowAnyAuthenticatedClient::new(root_store),
        CIPHER_SUITES,
    );

    tls_server_config
        .set_single_cert(vec![cert.clone()], cert_priv_key.clone())
        .map_err(TransportError::other)?;
    // put this in a database at some point
    tls_server_config.set_persistence(rustls::ServerSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    ));
    tls_server_config.ticketer = rustls::Ticketer::new();
    tls_server_config.set_protocols(&[ALPN_KITSUNE_PROXY_0.to_vec()]);
    let tls_server_config = Arc::new(tls_server_config);

    let mut tls_client_config = rustls::ClientConfig::with_ciphersuites(CIPHER_SUITES);
    tls_client_config
        .set_single_client_cert(vec![cert], cert_priv_key)
        .map_err(TransportError::other)?;
    tls_client_config
        .dangerous()
        .set_certificate_verifier(TlsServerVerifier::new());
    // put this in a database at some point
    tls_client_config.set_persistence(rustls::ClientSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    ));
    tls_client_config.set_protocols(&[ALPN_KITSUNE_PROXY_0.to_vec()]);
    let tls_client_config = Arc::new(tls_client_config);

    Ok((tls_server_config, tls_client_config))
}

struct TlsServerVerifier;

impl TlsServerVerifier {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::ServerCertVerifier for TlsServerVerifier {
    fn verify_server_cert(
        &self,
        _roots: &rustls::RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: webpki::DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        // TODO - check acceptable cert digest

        Ok(rustls::ServerCertVerified::assertion())
    }
}
