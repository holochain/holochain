use crate::*;

/// Tls ALPN identifier for kitsune proxy handshaking
const ALPN_KITSUNE_PROXY_0: &[u8] = b"kitsune-proxy/0";

/// Allow only these cipher suites for kitsune proxy Tls.
static CIPHER_SUITES: &[&rustls::SupportedCipherSuite] = &[
    &rustls::ciphersuite::TLS13_CHACHA20_POLY1305_SHA256,
    &rustls::ciphersuite::TLS13_AES_256_GCM_SHA384,
];

/// Wrap a transport listener sender/receiver in kitsune proxy logic.
pub async fn spawn_kitsune_proxy_listener(
    proxy_config: Arc<ProxyConfig>,
    sub_sender: ghost_actor::GhostSender<TransportListener>,
    sub_receiver: TransportListenerEventReceiver,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportListenerEventReceiver,
)> {
    let (tls, accept_proxy_cb, proxy_url): (TlsConfig, AcceptProxyCallback, Option<ProxyUrl>) =
        match proxy_config.as_ref() {
            ProxyConfig::RemoteProxyClient { tls, proxy_url } => (
                tls.clone(),
                AcceptProxyCallback::reject_all(),
                Some(proxy_url.clone()),
            ),
            ProxyConfig::LocalProxyServer {
                tls,
                accept_proxy_cb,
            } => (tls.clone(), accept_proxy_cb.clone(), None),
        };

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory
        .create_channel::<TransportListener>()
        .await?;

    channel_factory.attach_receiver(sub_receiver).await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(
        builder.spawn(
            InnerListen::new(
                channel_factory,
                tls,
                accept_proxy_cb,
                sub_sender,
                evt_send,
                proxy_url,
            )
            .await?,
        ),
    );

    Ok((sender, evt_recv))
}

pub(crate) fn gen_tls_configs(
    tls: &TlsConfig,
) -> TransportResult<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>)> {
    let cert = rustls::Certificate(tls.cert.0.to_vec());
    let cert_priv_key = rustls::PrivateKey(tls.cert_priv_key.0.to_vec());

    let mut tls_server_config =
        rustls::ServerConfig::with_ciphersuites(rustls::NoClientAuth::new(), CIPHER_SUITES);
    tls_server_config
        .set_single_cert(vec![cert], cert_priv_key)
        .map_err(TransportError::other)?;
    tls_server_config.set_protocols(&[ALPN_KITSUNE_PROXY_0.to_vec()]);
    let tls_server_config = Arc::new(tls_server_config);

    let mut tls_client_config = rustls::ClientConfig::with_ciphersuites(CIPHER_SUITES);
    tls_client_config
        .dangerous()
        .set_certificate_verifier(TlsServerVerifier::new());
    tls_client_config.set_protocols(&[ALPN_KITSUNE_PROXY_0.to_vec()]);
    let tls_client_config = Arc::new(tls_client_config);

    Ok((tls_server_config, tls_client_config))
}

#[allow(dead_code)]
struct InnerListen {
    channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
    accept_proxy_cb: AcceptProxyCallback,
    sub_sender: ghost_actor::GhostSender<TransportListener>,
    evt_send: futures::channel::mpsc::Sender<TransportListenerEvent>,
    tls: TlsConfig,
    tls_server_config: Arc<rustls::ServerConfig>,
    tls_client_config: Arc<rustls::ClientConfig>,
}

impl InnerListen {
    pub async fn new(
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        tls: TlsConfig,
        accept_proxy_cb: AcceptProxyCallback,
        sub_sender: ghost_actor::GhostSender<TransportListener>,
        evt_send: futures::channel::mpsc::Sender<TransportListenerEvent>,
        _proxy_url: Option<ProxyUrl>,
    ) -> TransportResult<Self> {
        let (tls_server_config, tls_client_config) = gen_tls_configs(&tls)?;

        let out = Self {
            channel_factory,
            accept_proxy_cb,
            sub_sender,
            evt_send,
            tls,
            tls_server_config,
            tls_client_config,
        };

        /*
        if let Some(proxy_url) = proxy_url {
            let (_proxy_url, _proxy_send, _proxy_recv) =
                out.negotiate_proxy_service(proxy_url).await?;
        }
        */

        Ok(out)
    }

    /*
    /// low-level transport connect
    fn low_level_connect(
        &self,
        proxy_url: &ProxyUrl,
    ) -> TransportListenerHandlerResult<(
        ghost_actor::GhostSender<TlsConnection>,
        TlsConnectionReceiver,
    )> {
        let base_url = proxy_url.as_base().clone();

        let tls = self.tls.clone();
        let tls_server_config = self.tls_server_config.clone();
        let tls_client_config = self.tls_client_config.clone();

        let fut = self.sub_sender.connect(base_url);
        Ok(async move {
            let (sender, receiver) = fut.await?;
            let (sender, receiver) =
                spawn_tls_connection(sender, receiver, tls, tls_server_config, tls_client_config)
                    .await?;
            Ok((sender, receiver))
        }
        .boxed()
        .into())
    }

    /// Request that a remote transport proxy for us.
    async fn negotiate_proxy_service(
        &mut self,
        proxy_url: ProxyUrl,
    ) -> TransportResult<(
        ProxyUrl,
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        let (sender, receiver) = self.low_level_connect(&proxy_url)?.await?;
        let proxy_url = sender.req_proxy().await?;
        let (sender, receiver) = spawn_kitsune_proxy_connection(sender, receiver).await?;
        Ok(((*proxy_url).clone(), sender, receiver))
    }
    */
}

impl ghost_actor::GhostControlHandler for InnerListen {
    fn handle_ghost_actor_shutdown(self) -> MustBoxFuture<'static, ()> {
        async move {
            let _ = self.sub_sender.ghost_actor_shutdown().await;
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<TransportListener> for InnerListen {}

impl TransportListenerHandler for InnerListen {
    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        // TODO translate url
        let fut = self.sub_sender.bound_url();
        Ok(async move { fut.await }.boxed().into())
    }

    fn handle_connect(
        &mut self,
        _url: url2::Url2,
    ) -> TransportListenerHandlerResult<(
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        /*
        let proxy_url: ProxyUrl = url.into();
        let fut = self.low_level_connect(&proxy_url)?;
        Ok(async move {
            let (sender, receiver) = fut.await?;
            let (sender, receiver) = spawn_kitsune_proxy_connection(sender, receiver).await?;
            Ok((sender, receiver))
        }
        .boxed()
        .into())
        */
        unimplemented!()
    }
}

impl ghost_actor::GhostHandler<TransportConnectionEvent> for InnerListen {}

impl TransportConnectionEventHandler for InnerListen {
    fn handle_incoming_channel(
        &mut self,
        _url: url2::Url2,
        _send: TransportChannelWrite,
        _recv: TransportChannelRead,
    ) -> TransportConnectionEventHandlerResult<()> {
        unimplemented!()
    }
}

impl ghost_actor::GhostHandler<TransportListenerEvent> for InnerListen {}

impl TransportListenerEventHandler for InnerListen {
    fn handle_incoming_connection(
        &mut self,
        _sender: ghost_actor::GhostSender<TransportConnection>,
        receiver: TransportConnectionEventReceiver,
    ) -> TransportListenerEventHandlerResult<()> {
        let fut = self.channel_factory.attach_receiver(receiver);

        Ok(async move {
            let _ = fut.await;
            Ok(())
        }
        .boxed()
        .into())
        /*
        let accept_proxy_cb = self.accept_proxy_cb.clone();
        let evt_send = self.evt_send.clone();
        let tls = self.tls.clone();
        let tls_server_config = self.tls_server_config.clone();
        let tls_client_config = self.tls_client_config.clone();
        Ok(async move {
            let (sender, receiver) =
                spawn_tls_connection(sender, receiver, tls, tls_server_config, tls_client_config)
                    .await?;
            let (sender, receiver) = spawn_kitsune_proxy_connection(sender, receiver).await?;
            // TODO - NOPE!!! this is fake! move to proxy handler
            if !accept_proxy_cb(vec![0; 32].into()).await {
                // TODO - send back an error wire
                sender.ghost_actor_shutdown().await?;
                return Ok(());
            }
            evt_send.incoming_connection(sender, receiver).await?;
            Ok(())
        }
        .boxed()
        .into())
        */
        //unimplemented!()
    }
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
