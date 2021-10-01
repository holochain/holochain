//! TLS utils for kitsune

use crate::config::*;
use crate::*;
use legacy_lair_api::actor::*;

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
        let mut options = legacy_lair_api::actor::TlsCertOptions::default();
        options.alg = legacy_lair_api::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
        let cert = legacy_lair_api::internal::tls::tls_cert_self_signed_new_from_entropy(options)
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
static CIPHER_SUITES: &[&rustls::SupportedCipherSuite] = &[
    &rustls::ciphersuite::TLS13_CHACHA20_POLY1305_SHA256,
    &rustls::ciphersuite::TLS13_AES_256_GCM_SHA384,
];

/// Helper to generate rustls configs given a TlsConfig reference.
#[allow(dead_code)]
pub fn gen_tls_configs(
    alpn: &[u8],
    tls: &TlsConfig,
    tuning_params: KitsuneP2pTuningParams,
) -> KitsuneResult<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>)> {
    let cert = rustls::Certificate(tls.cert.0.to_vec());
    let cert_priv_key = rustls::PrivateKey(tls.cert_priv_key.0.to_vec());

    let root_cert = rustls::Certificate(legacy_lair_api::internal::tls::WK_CA_CERT_DER.to_vec());
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(&root_cert).unwrap();

    let mut tls_server_config = rustls::ServerConfig::with_ciphersuites(
        rustls::AllowAnyAuthenticatedClient::new(root_store),
        CIPHER_SUITES,
    );

    tls_server_config
        .set_single_cert(vec![cert.clone()], cert_priv_key.clone())
        .map_err(KitsuneError::other)?;

    // put this in a database at some point
    tls_server_config.set_persistence(rustls::ServerSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    ));
    tls_server_config.ticketer = rustls::Ticketer::new();
    tls_server_config.set_protocols(&[alpn.to_vec()]);
    tls_server_config.versions = vec![rustls::ProtocolVersion::TLSv1_3];
    let tls_server_config = Arc::new(tls_server_config);

    let mut tls_client_config = rustls::ClientConfig::with_ciphersuites(CIPHER_SUITES);
    tls_client_config
        .set_single_client_cert(vec![cert], cert_priv_key)
        .map_err(KitsuneError::other)?;
    tls_client_config
        .dangerous()
        .set_certificate_verifier(TlsServerVerifier::new());

    // put this in a database at some point
    tls_client_config.set_persistence(rustls::ClientSessionMemoryCache::new(
        tuning_params.tls_in_mem_session_storage as usize,
    ));
    tls_client_config.set_protocols(&[alpn.to_vec()]);
    tls_client_config.versions = vec![rustls::ProtocolVersion::TLSv1_3];
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
