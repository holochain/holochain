use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};
use ghost_actor::dependencies::tracing;
use std::collections::HashMap;

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

    let this_url = sub_sender.bound_url().await?;
    let this_url = ProxyUrl::new(this_url.as_str(), tls.cert_digest.clone())?;

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory
        .create_channel::<TransportListener>()
        .await?;

    let i_s = channel_factory.create_channel::<Internal>().await?;

    channel_factory.attach_receiver(sub_receiver).await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(
        builder.spawn(
            InnerListen::new(
                channel_factory,
                i_s.clone(),
                this_url,
                tls,
                accept_proxy_cb,
                sub_sender,
                evt_send,
            )
            .await?,
        ),
    );

    if let Some(proxy_url) = proxy_url {
        i_s.req_proxy(proxy_url).await?;
    }

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
    i_s: ghost_actor::GhostSender<Internal>,
    this_url: ProxyUrl,
    accept_proxy_cb: AcceptProxyCallback,
    sub_sender: ghost_actor::GhostSender<TransportListener>,
    evt_send: futures::channel::mpsc::Sender<TransportListenerEvent>,
    tls: TlsConfig,
    tls_server_config: Arc<rustls::ServerConfig>,
    tls_client_config: Arc<rustls::ClientConfig>,
    low_level_connections: HashMap<url2::Url2, ghost_actor::GhostSender<TransportConnection>>,
}

impl InnerListen {
    pub async fn new(
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        i_s: ghost_actor::GhostSender<Internal>,
        this_url: ProxyUrl,
        tls: TlsConfig,
        accept_proxy_cb: AcceptProxyCallback,
        sub_sender: ghost_actor::GhostSender<TransportListener>,
        evt_send: futures::channel::mpsc::Sender<TransportListenerEvent>,
    ) -> TransportResult<Self> {
        tracing::info!(
            "{}: starting up with this_url: {}",
            this_url.short(),
            this_url
        );

        let (tls_server_config, tls_client_config) = gen_tls_configs(&tls)?;

        Ok(Self {
            channel_factory,
            i_s,
            this_url,
            accept_proxy_cb,
            sub_sender,
            evt_send,
            tls,
            tls_server_config,
            tls_client_config,
            low_level_connections: HashMap::new(),
        })
    }
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

ghost_actor::ghost_chan! {
    chan Internal<TransportError> {
        fn assert_low_level_connection(
            base_url: url2::Url2,
        ) -> ghost_actor::GhostSender<TransportConnection>;

        fn create_low_level_channel(
            base_url: url2::Url2,
        ) -> (
            futures::channel::mpsc::Sender<ProxyWire>,
            futures::channel::mpsc::Receiver<ProxyWire>,
        );

        fn register_low_level_connection(
            base_url: url2::Url2,
            sender: ghost_actor::GhostSender<TransportConnection>,
            receiver: futures::channel::mpsc::Receiver<TransportConnectionEvent>,
        ) -> ghost_actor::GhostSender<TransportConnection>;

        fn req_proxy(proxy_url: ProxyUrl) -> ();
        fn set_proxy_url(proxy_url: ProxyUrl) -> ();
    }
}

impl ghost_actor::GhostHandler<Internal> for InnerListen {}

impl InternalHandler for InnerListen {
    fn handle_assert_low_level_connection(
        &mut self,
        base_url: url2::Url2,
    ) -> InternalHandlerResult<ghost_actor::GhostSender<TransportConnection>> {
        if let Some(con) = self.low_level_connections.get(&base_url) {
            let con = con.clone();
            return Ok(async move { Ok(con) }.boxed().into());
        }
        let short = self.this_url.short().to_string();
        tracing::debug!("{}: connecting to {}", short, base_url);
        let fut = self.sub_sender.connect(base_url);
        let i_s = self.i_s.clone();
        Ok(async move {
            let (send, recv) = fut.await?;
            let base_url = send.remote_url().await?;
            let con = i_s
                .register_low_level_connection(base_url.clone(), send, recv)
                .await?;
            tracing::debug!("{}: CONNECTED to {}", short, base_url);
            Ok(con)
        }
        .boxed()
        .into())
    }

    fn handle_create_low_level_channel(
        &mut self,
        base_url: url2::Url2,
    ) -> InternalHandlerResult<(
        futures::channel::mpsc::Sender<ProxyWire>,
        futures::channel::mpsc::Receiver<ProxyWire>,
    )> {
        let short = self.this_url.short().to_string();
        let con = self.i_s.assert_low_level_connection(base_url.clone());
        Ok(async move {
            let con = con.await?;
            tracing::trace!("{}: low-level channel to {}", short, base_url);
            let (write, read) = con.create_channel().await?;
            let write = wire_write::wrap_wire_write(write);
            let read = wire_read::wrap_wire_read(read);
            tracing::trace!("{}: CHANNEL to {}", short, base_url);
            Ok((write, read))
        }
        .boxed()
        .into())
    }

    fn handle_register_low_level_connection(
        &mut self,
        base_url: url2::Url2,
        sender: ghost_actor::GhostSender<TransportConnection>,
        receiver: futures::channel::mpsc::Receiver<TransportConnectionEvent>,
    ) -> InternalHandlerResult<ghost_actor::GhostSender<TransportConnection>> {
        if let Some(con) = self.low_level_connections.get(&base_url) {
            let con = con.clone();
            return Ok(async move { Ok(con) }.boxed().into());
        }
        self.low_level_connections.insert(base_url, sender.clone());
        let fut = self.channel_factory.attach_receiver(receiver);
        tokio::task::spawn(fut);
        Ok(async move { Ok(sender) }.boxed().into())
    }

    fn handle_req_proxy(&mut self, proxy_url: ProxyUrl) -> InternalHandlerResult<()> {
        tracing::info!(
            "{}: wishes to proxy through {}:{}",
            self.this_url.short(),
            proxy_url.short(),
            proxy_url
        );
        let fut = self.i_s.create_low_level_channel(proxy_url.into_base());
        let i_s = self.i_s.clone();
        Ok(async move {
            let (mut write, mut read) = fut.await?;

            write
                .send(ProxyWire::req_proxy(MsgId::next()))
                .await
                .map_err(TransportError::other)?;
            let res = match read.next().await {
                None => return Err("no response to proxy request".into()),
                Some(r) => r,
            };
            let proxy_url = match res {
                ProxyWire::ReqProxyOk(ReqProxyOk(_msg_id, proxy_url)) => proxy_url,
                ProxyWire::ReqProxyErr(ReqProxyErr(_msg_id, reason)) => {
                    return Err(format!("err response to proxy request: {:?}", reason).into());
                }
                _ => return Err(format!("unexpected: {:?}", res).into()),
            };
            i_s.set_proxy_url(proxy_url.into()).await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_set_proxy_url(&mut self, proxy_url: ProxyUrl) -> InternalHandlerResult<()> {
        self.this_url = proxy_url;
        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<TransportListener> for InnerListen {}

impl TransportListenerHandler for InnerListen {
    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        let this_url: url2::Url2 = (&self.this_url).into();
        Ok(async move { Ok(this_url) }.boxed().into())
    }

    fn handle_connect(
        &mut self,
        url: url2::Url2,
    ) -> TransportListenerHandlerResult<(
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        let proxy_url = ProxyUrl::from(url);
        let fut = self
            .i_s
            .assert_low_level_connection(proxy_url.as_base().clone());

        Ok(async move {
            let _con = fut.await?;
            unimplemented!()
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<TransportConnectionEvent> for InnerListen {}

impl TransportConnectionEventHandler for InnerListen {
    fn handle_incoming_channel(
        &mut self,
        _url: url2::Url2,
        write: TransportChannelWrite,
        read: TransportChannelRead,
    ) -> TransportConnectionEventHandlerResult<()> {
        //let i_s = self.i_s.clone();
        let this_url = self.this_url.clone();
        let mut write = wire_write::wrap_wire_write(write);
        let mut read = wire_read::wrap_wire_read(read);

        tokio::task::spawn(async move {
            while let Some(wire) = read.next().await {
                match wire {
                    ProxyWire::ReqProxy(ReqProxy(msg_id)) => {
                        // TODO set cert to match remote
                        let proxy_url = this_url.clone();
                        // TODO always agreeing for now
                        write
                            .send(ProxyWire::req_proxy_ok(msg_id, proxy_url.into()))
                            .await
                            .map_err(TransportError::other)?;
                        write.close().await.map_err(TransportError::other)?;
                    }
                    /*
                    ProxyWire::ChanNew(ChanNew(msg_id, proxy_url)) => {
                        if proxy_url == this_url {
                            panic!("cannot handle not passthru yet")
                        }

                    }
                    */
                    _ => panic!("unexpected: {:?}", wire),
                }
            }
            TransportResult::Ok(())
        });

        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<TransportListenerEvent> for InnerListen {}

impl TransportListenerEventHandler for InnerListen {
    fn handle_incoming_connection(
        &mut self,
        sender: ghost_actor::GhostSender<TransportConnection>,
        receiver: TransportConnectionEventReceiver,
    ) -> TransportListenerEventHandlerResult<()> {
        let short = self.this_url.short().to_string();
        let i_s = self.i_s.clone();
        Ok(async move {
            let base_url = sender.remote_url().await?;
            tracing::debug!("{}: INCOMING CONNECTION from {}", short, base_url);
            i_s.register_low_level_connection(base_url.clone(), sender, receiver)
                .await?;
            tracing::debug!("{}: INCOMING CONNECTION done {}", short, base_url);

            Ok(())
        }
        .boxed()
        .into())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn init_tracing() {
        let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_inner_listen() {
        if let Err(e) = test_inner_listen_inner().await {
            panic!("{:?}", e);
        }
    }

    async fn connect(
        proxy_config: Arc<ProxyConfig>,
    ) -> TransportResult<ghost_actor::GhostSender<TransportListener>> {
        let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
        let addr = bind.bound_url().await?;
        tracing::warn!("got bind: {}", addr);

        let (bind, _evt) = spawn_kitsune_proxy_listener(proxy_config, bind, evt).await?;
        let addr = bind.bound_url().await?;
        tracing::warn!("got proxy: {}", addr);

        Ok(bind)
    }

    async fn test_inner_listen_inner() -> TransportResult<()> {
        init_tracing();

        let proxy_config1 = ProxyConfig::local_proxy_server(
            TlsConfig::new_ephemeral().await?,
            AcceptProxyCallback::accept_all(),
        );
        let bind1 = connect(proxy_config1).await?;
        let addr1 = bind1.bound_url().await?;

        let proxy_config2 = ProxyConfig::local_proxy_server(
            TlsConfig::new_ephemeral().await?,
            AcceptProxyCallback::accept_all(),
        );
        let bind2 = connect(proxy_config2).await?;

        let proxy_config3 =
            ProxyConfig::remote_proxy_client(TlsConfig::new_ephemeral().await?, addr1.into());
        let bind3 = connect(proxy_config3).await?;
        let addr3 = bind3.bound_url().await?;

        let (_send, _recv) = bind2.connect(addr3).await?;

        Ok(())
    }
}
