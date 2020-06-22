use futures::{future::FutureExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, url2::*},
    transport::transport_connection::*,
    transport::transport_listener::*,
    transport::*,
};

ghost_actor::ghost_actor! {
    actor ListenerInner<TransportError> {
        /// our incoming task has produced a connection instance
        fn register_incoming(
            sender: ghost_actor::GhostSender<TransportConnection>,
            receiver: TransportConnectionEventReceiver,
        ) -> ();
    }
}

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    #[allow(dead_code)]
    internal_sender: ghost_actor::GhostSender<ListenerInner>,
    quinn_endpoint: quinn::Endpoint,
    incoming_sender: futures::channel::mpsc::Sender<TransportListenerEvent>,
}

impl ghost_actor::GhostControlHandler for TransportListenerQuic {}

impl ghost_actor::GhostHandler<ListenerInner> for TransportListenerQuic {}

impl ListenerInnerHandler for TransportListenerQuic {
    fn handle_register_incoming(
        &mut self,
        sender: ghost_actor::GhostSender<TransportConnection>,
        receiver: TransportConnectionEventReceiver,
    ) -> ListenerInnerHandlerResult<()> {
        let send_clone = self.incoming_sender.clone();
        Ok(
            async move { send_clone.incoming_connection(sender, receiver).await }
                .boxed()
                .into(),
        )
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
                .map_err(TransportError::custom)?,
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
        // TODO fix this block_on
        let addr = tokio_safe_block_on::tokio_safe_block_on(
            crate::url_to_addr(&input, crate::SCHEME),
            std::time::Duration::from_secs(1),
        )
        .unwrap()?;
        let maybe_con = self
            .quinn_endpoint
            .connect(&addr, "stub.stub")
            .map_err(TransportError::custom)?;
        Ok(
            async move { crate::connection::spawn_transport_connection_quic(maybe_con).await }
                .boxed()
                .into(),
        )
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
        .map_err(|e| TransportError::from(format!("cert error: {:?}", e)))?;
    let mut builder = quinn::Endpoint::builder();
    builder.listen(server_config);
    builder.default_client_config(danger::configure_client());
    let (quinn_endpoint, mut incoming) = builder
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::custom)?;

    let (incoming_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder.channel_factory().create_channel().await?;

    let sender = builder.channel_factory().create_channel().await?;

    let internal_sender_clone = internal_sender.clone();
    tokio::task::spawn(async move {
        while let Some(maybe_con) = incoming.next().await {
            let internal_sender_clone = internal_sender_clone.clone();

            // TODO - some buffer_unordered(10) magic
            //        so we don't process infinite incoming connections
            tokio::task::spawn(async move {
                let r = match crate::connection::spawn_transport_connection_quic(maybe_con).await {
                    Err(_) => {
                        // TODO - log this?
                        return;
                    }
                    Ok(r) => r,
                };

                if let Err(_) = internal_sender_clone.register_incoming(r.0, r.1).await {
                    // TODO - log this?
                    return;
                }
            });
        }
    });

    let actor = TransportListenerQuic {
        internal_sender,
        quinn_endpoint,
        incoming_sender,
    };

    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}

mod danger {
    use quinn::{
        Certificate, CertificateChain, ClientConfig, ClientConfigBuilder, PrivateKey, ServerConfig,
        ServerConfigBuilder, TransportConfig,
    };
    use std::sync::Arc;

    #[allow(dead_code)]
    pub(crate) fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn std::error::Error>>
    {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_der = cert.serialize_der().unwrap();
        let priv_key = cert.serialize_private_key_der();
        let priv_key = PrivateKey::from_der(&priv_key)?;

        let mut transport_config = TransportConfig::default();
        transport_config.stream_window_uni(0);
        let mut server_config = ServerConfig::default();
        server_config.transport = Arc::new(transport_config);
        let mut cfg_builder = ServerConfigBuilder::new(server_config);
        let cert = Certificate::from_der(&cert_der)?;
        cfg_builder.certificate(CertificateChain::from_certs(vec![cert]), priv_key)?;

        Ok((cfg_builder.build(), cert_der))
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
