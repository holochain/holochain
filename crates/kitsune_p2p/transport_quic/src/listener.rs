use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::transport_listener::*,
    transport::*,
};
use std::net::SocketAddr;

ghost_actor::ghost_chan! {
    chan ListenerInner<TransportError> {
        /// internal raw connect fn
        fn raw_connect(addr: SocketAddr) -> quinn::Connecting;
    }
}

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    internal_sender: ghost_actor::GhostSender<ListenerInner>,
    quinn_endpoint: quinn::Endpoint,
}

impl ghost_actor::GhostControlHandler for TransportListenerQuic {}

impl ghost_actor::GhostHandler<ListenerInner> for TransportListenerQuic {}

impl ListenerInnerHandler for TransportListenerQuic {
    fn handle_raw_connect(
        &mut self,
        addr: SocketAddr,
    ) -> ListenerInnerHandlerResult<quinn::Connecting> {
        let out = self.quinn_endpoint.connect(&addr, "stub.stub").map_err(TransportError::other)?;
        Ok(async move { Ok(out) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<TransportListener> for TransportListenerQuic {}

impl TransportListenerHandler for TransportListenerQuic {
    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<Url2> {
        let out = url2!(
            "{}://{}",
            crate::SCHEME,
            self.quinn_endpoint
                .local_addr()
                .map_err(TransportError::other)?,
        );
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_connect(
        &mut self,
        input: Url2,
    ) -> TransportListenerHandlerResult<(
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        let i_s = self.internal_sender.clone();
        Ok(async move {
            let addr = crate::url_to_addr(&input, crate::SCHEME).await?;
            let maybe_con = i_s.raw_connect(addr).await?;
            crate::connection::spawn_transport_connection_quic(maybe_con).await
        }.boxed().into())
    }
}

/// Spawn a new QUIC TransportListenerSender.
pub async fn spawn_transport_listener_quic(
    bind_to: Url2,
) -> TransportListenerResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportListenerEventReceiver,
)> {
    let (server_config, _server_cert) = danger::configure_server()
        .await
        .map_err(|e| TransportError::from(format!("cert error: {:?}", e)))?;
    let mut builder = quinn::Endpoint::builder();
    builder.listen(server_config);
    builder.default_client_config(danger::configure_client());
    let (quinn_endpoint, incoming) = builder
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::other)?;

    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder.channel_factory().create_channel().await?;

    let sender = builder.channel_factory().create_channel().await?;

    tokio::task::spawn(async move {
        incoming.for_each_concurrent(10, |maybe_con| async {
            let res: TransportResult<()> = async {
                let (con_send, con_recv) = crate::connection::spawn_transport_connection_quic(maybe_con).await?;
                incoming_sender.incoming_connection(con_send, con_recv).await?;

                Ok(())
            }.await;
            if let Err(err) = res {
                ghost_actor::dependencies::tracing::error!(?err);
            }
        }).await;
    });

    let actor = TransportListenerQuic {
        internal_sender,
        quinn_endpoint,
    };

    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}

mod danger {
    use kitsune_p2p_types::transport::{TransportResult, TransportError};
    use quinn::{
        Certificate, CertificateChain, ClientConfig, ClientConfigBuilder, PrivateKey, ServerConfig,
        ServerConfigBuilder, TransportConfig,
    };
    use std::sync::Arc;

    #[allow(dead_code)]
    pub(crate) async fn configure_server() -> TransportResult<(ServerConfig, Vec<u8>)>
    {
        //let options = lair_keystore_api::actor::TlsCertOptions::default();
        //let cert = lair_keystore_api::internal::tls::tls_cert_self_signed_new_from_entropy(
        //    lair_keystore_api::actor::TlsCertOptions::default(),
        //).await.map_err(TransportError::other)?;
        let sni = format!("a{}a.a{}a", nanoid::nanoid!(), nanoid::nanoid!());
        let mut params = rcgen::CertificateParams::new(vec![sni.clone()]);
        //params.alg = &rcgen::PKCS_ED25519;
        //params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;
        params.alg = &rcgen::PKCS_ECDSA_P384_SHA384;
        let cert = rcgen::Certificate::from_params(params)
            .map_err(TransportError::other)?;
        let priv_key_der = cert.serialize_private_key_der();
        let cert_der = cert.serialize_der().map_err(TransportError::other)?;
        let cert = lair_keystore_api::entry::EntryTlsCert {
            sni: sni.into(),
            priv_key_der: priv_key_der.into(),
            cert_der: cert_der.into(),
            cert_digest: vec![0; 32].into(),
        };

        let priv_key = PrivateKey::from_der(&cert.priv_key_der).map_err(TransportError::other)?;

        let mut transport_config = TransportConfig::default();
        transport_config.stream_window_uni(0);
        let mut server_config = ServerConfig::default();
        server_config.transport = Arc::new(transport_config);
        let mut cfg_builder = ServerConfigBuilder::new(server_config);
        let tcert = Certificate::from_der(&cert.cert_der).map_err(TransportError::other)?;
        cfg_builder.certificate(CertificateChain::from_certs(vec![tcert]), priv_key).map_err(TransportError::other)?;

        Ok((cfg_builder.build(), cert.cert_der.to_vec()))
    }

    /// Dummy certificate verifier that treats any certificate as valid.
    /// NOTE, such verification is vulnerable to MITM attacks, but convenient for testing.
    struct SkipServerVerification;

    impl SkipServerVerification {
        fn new() -> Arc<Self> {
            Arc::new(Self)
        }
    }

    impl rustls::ServerCertVerifier for SkipServerVerification {
        fn verify_server_cert(
            &self,
            _roots: &rustls::RootCertStore,
            _presented_certs: &[rustls::Certificate],
            _dns_name: webpki::DNSNameRef,
            _ocsp_response: &[u8],
        ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
            Ok(rustls::ServerCertVerified::assertion())
        }
    }

    pub(crate) fn configure_client() -> ClientConfig {
        let mut cfg = ClientConfigBuilder::default().build();
        let tls_cfg: &mut rustls::ClientConfig = Arc::get_mut(&mut cfg.crypto).unwrap();
        // this is only available when compiled with "dangerous_configuration" feature
        tls_cfg
            .dangerous()
            .set_certificate_verifier(SkipServerVerification::new());
        cfg
    }
}
