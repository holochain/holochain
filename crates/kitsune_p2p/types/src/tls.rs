//! TLS utils for kitsune

use crate::config::*;
use crate::*;
use lair_keystore_api_0_0::actor::*;
use once_cell::sync::Lazy;

/// Tls Configuration.
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
    pub async fn new_ephemeral() -> KitsuneResult<Self> {
        let mut options = lair_keystore_api_0_0::actor::TlsCertOptions::default();
        options.alg = lair_keystore_api_0_0::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
        let cert =
            lair_keystore_api_0_0::internal::tls::tls_cert_self_signed_new_from_entropy(options)
                .await
                .map_err(KitsuneError::other)?;
        Ok(Self {
            cert: cert.cert_der,
            cert_priv_key: cert.priv_key_der,
            cert_digest: cert.cert_digest,
        })
    }
}

/// Allow only these cipher suites for kitsune Tls.
static CIPHER_SUITES: &[rustls::SupportedCipherSuite] = &[
    rustls::cipher_suite::TLS13_CHACHA20_POLY1305_SHA256,
    rustls::cipher_suite::TLS13_AES_256_GCM_SHA384,
];

/// Single shared keylog file all sessions can report to
static KEY_LOG: Lazy<Arc<dyn rustls::KeyLog>> = Lazy::new(|| {
    println!("ONTHEUNOE {:?}", std::env::var("SSLKEYLOGFILE"));
    Arc::new(rustls::KeyLogFile::new())
});

/// Helper to generate rustls configs given a TlsConfig reference.
#[allow(dead_code)]
pub fn gen_tls_configs(
    alpn: &[u8],
    tls: &TlsConfig,
    tuning_params: KitsuneP2pTuningParams,
) -> KitsuneResult<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>)> {
    let cert = rustls::Certificate(tls.cert.0.to_vec());
    let cert_priv_key = rustls::PrivateKey(tls.cert_priv_key.0.to_vec());

    let root_cert =
        rustls::Certificate(lair_keystore_api_0_0::internal::tls::WK_CA_CERT_DER.to_vec());
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(&root_cert).unwrap();

    let mut tls_server_config = rustls::ServerConfig::builder()
        .with_cipher_suites(CIPHER_SUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(KitsuneError::other)?
        .with_client_cert_verifier(rustls::server::AllowAnyAuthenticatedClient::new(root_store))
        .with_single_cert(vec![cert.clone()], cert_priv_key.clone())
        .map_err(KitsuneError::other)?;

    if tuning_params.use_env_tls_keylog() {
        tls_server_config.key_log = KEY_LOG.clone();
    }
    tls_server_config.ticketer = rustls::Ticketer::new().map_err(KitsuneError::other)?;
    tls_server_config.session_storage = rustls::server::ServerSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    );
    tls_server_config.alpn_protocols.push(alpn.to_vec());

    let tls_server_config = Arc::new(tls_server_config);

    let mut tls_client_config = rustls::ClientConfig::builder()
        .with_cipher_suites(CIPHER_SUITES)
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(KitsuneError::other)?
        .with_custom_certificate_verifier(TlsServerVerifier::new())
        .with_single_cert(vec![cert], cert_priv_key)
        .map_err(KitsuneError::other)?;

    if tuning_params.use_env_tls_keylog() {
        tls_client_config.key_log = KEY_LOG.clone();
    }
    tls_client_config.session_storage = rustls::client::ClientSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    );
    tls_client_config.alpn_protocols.push(alpn.to_vec());

    let tls_client_config = Arc::new(tls_client_config);

    Ok((tls_server_config, tls_client_config))
}

struct TlsServerVerifier;

impl TlsServerVerifier {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for TlsServerVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        // TODO - check acceptable cert digest

        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
