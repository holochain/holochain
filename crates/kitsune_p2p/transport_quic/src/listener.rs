use crate::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::ghost_actor::GhostControlSender;
use kitsune_p2p_types::dependencies::serde_json;
use kitsune_p2p_types::dependencies::url2;
use kitsune_p2p_types::transport::*;
use std::collections::HashMap;
use std::net::SocketAddr;

struct DropMe(tokio::sync::OwnedSemaphorePermit, std::time::Instant);

impl DropMe {
    pub fn new(permit: tokio::sync::OwnedSemaphorePermit) -> Arc<Self> {
        Arc::new(Self(permit, std::time::Instant::now()))
    }
}

static LONG_CHAN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
impl Drop for DropMe {
    fn drop(&mut self) {
        let time = self.1.elapsed().as_millis();
        if time > 500 && LONG_CHAN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 10_000 == 0
        {
            tracing::warn!("10_000 slow channel times, e.g.: {} ms", time);
        }
    }
}

struct TrackedTransportChannelWrite {
    write: TransportChannelWrite,
    _drop_me: Arc<DropMe>,
}

impl futures::sink::Sink<Vec<u8>> for TrackedTransportChannelWrite {
    type Error = TransportError;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let write = &mut self.write;
        tokio::pin!(write);
        futures::sink::Sink::poll_ready(write, cx)
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        let write = &mut self.write;
        tokio::pin!(write);
        futures::sink::Sink::start_send(write, item)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let write = &mut self.write;
        tokio::pin!(write);
        futures::sink::Sink::poll_flush(write, cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let write = &mut self.write;
        tokio::pin!(write);
        futures::sink::Sink::poll_close(write, cx)
    }
}

struct TrackedTransportChannelRead {
    read: TransportChannelRead,
    _drop_me: Arc<DropMe>,
}

impl futures::stream::Stream for TrackedTransportChannelRead {
    type Item = Vec<u8>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let read = &mut self.read;
        tokio::pin!(read);
        futures::stream::Stream::poll_next(read, cx)
    }
}

/// Convert quinn async read/write streams into Vec<u8> senders / receivers.
/// Quic bi-streams are Async Read/Write - But the kitsune transport api
/// uses Vec<u8> Streams / Sinks - This code translates into that.
fn tx_bi_chan(
    mut bi_send: quinn::SendStream,
    mut bi_recv: quinn::RecvStream,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> (TransportChannelWrite, TransportChannelRead) {
    let (write_send, mut write_recv) = futures::channel::mpsc::channel::<Vec<u8>>(10);
    let write_send = write_send.sink_map_err(TransportError::other);
    metric_task(async move {
        while let Some(data) = write_recv.next().await {
            bi_send
                .write_all(&data)
                .await
                .map_err(TransportError::other)?;
        }
        bi_send.finish().await.map_err(TransportError::other)?;
        TransportResult::Ok(())
    });
    let (mut read_send, read_recv) = futures::channel::mpsc::channel::<Vec<u8>>(10);
    metric_task(async move {
        let mut buf = [0_u8; 4096];
        while let Some(read) = bi_recv
            .read(&mut buf)
            .await
            .map_err(TransportError::other)?
        {
            if read == 0 {
                continue;
            }
            tracing::debug!("QUIC received {} bytes", read);
            read_send
                .send(buf[0..read].to_vec())
                .await
                .map_err(TransportError::other)?;
        }
        TransportResult::Ok(())
    });
    let drop_me = DropMe::new(permit);
    let write_send: TransportChannelWrite = Box::new(TrackedTransportChannelWrite {
        write: Box::new(write_send),
        _drop_me: drop_me.clone(),
    });
    let read_recv: TransportChannelRead = Box::new(TrackedTransportChannelRead {
        read: Box::new(read_recv),
        _drop_me: drop_me,
    });
    (write_send, read_recv)
}

#[derive(Clone)]
struct ConItem {
    pub con: quinn::Connection,
    pub channel_limit: Arc<tokio::sync::Semaphore>,
}

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    /// internal api logic
    internal_sender: ghost_actor::GhostSender<ListenerInner>,
    /// incoming channel send to our owner
    incoming_channel_sender: TransportEventSender,
    /// the url to return on 'bound_url' calls - what we bound to
    bound_url: Url2,
    /// the quinn binding (akin to a socket listener)
    quinn_endpoint: quinn::Endpoint,
    /// pool of active connections
    connections: HashMap<Url2, ConItem>,
    /// tuning params
    tuning_params: Arc<KitsuneP2pTuningParams>,
}

impl ghost_actor::GhostControlHandler for TransportListenerQuic {
    fn handle_ghost_actor_shutdown(
        mut self,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, ()> {
        async move {
            // Note: it's easiest to just blanket shut everything down.
            // If we wanted to be more graceful, we'd need to plumb
            // in some signals to start rejecting incoming connections,
            // then we could use `quinn_endpoint.wait_idle().await`.
            let _ = self.incoming_channel_sender.close_channel();
            for (_, con) in self.connections.into_iter() {
                con.con.close(0_u8.into(), b"");
                drop(con);
            }
            self.quinn_endpoint.close(0_u8.into(), b"");
        }
        .boxed()
        .into()
    }
}

ghost_actor::ghost_chan! {
    /// Internal Sender
    chan ListenerInner<TransportError> {
        /// Use our binding to establish a new outgoing connection.
        fn raw_connect(addr: SocketAddr) -> quinn::Connecting;

        /// Take a quinn connecting instance pulling it into our logic.
        /// Shared code for both incoming and outgoing connections.
        /// For outgoing create_channel we may also wish to create a channel.
        fn take_connecting(
            maybe_con: quinn::Connecting,
            with_channel: bool,
        ) -> Option<(
            Url2,
            TransportChannelWrite,
            TransportChannelRead,
        )>;

        /// Finalization step for taking control of a connection.
        /// Places it in our hash map for use establishing outgoing channels.
        fn set_connection(
            url: Url2,
            con: ConItem,
        ) -> ();

        /// If we get an error making outgoing channels,
        /// or if the incoming channel receiver stops,
        /// we want to remove this connection from our pool. It is done.
        fn drop_connection(url: Url2) -> ();
    }
}

impl ghost_actor::GhostHandler<ListenerInner> for TransportListenerQuic {}

impl ListenerInnerHandler for TransportListenerQuic {
    fn handle_raw_connect(
        &mut self,
        addr: SocketAddr,
    ) -> ListenerInnerHandlerResult<quinn::Connecting> {
        tracing::debug!("attempt raw connect: {:?}", addr);
        let out = self
            .quinn_endpoint
            .connect(&addr, "stub.stub")
            .map_err(TransportError::other)?;
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_take_connecting(
        &mut self,
        maybe_con: quinn::Connecting,
        with_channel: bool,
    ) -> ListenerInnerHandlerResult<Option<(Url2, TransportChannelWrite, TransportChannelRead)>>
    {
        let channel_limit = self.tuning_params.quic_connection_channel_limit;
        let i_s = self.internal_sender.clone();
        let mut incoming_channel_sender = self.incoming_channel_sender.clone();
        Ok(async move {
            // we only need to deal with the connection object
            // and the bi-streams receiver.
            let quinn::NewConnection {
                connection: con,
                mut bi_streams,
                ..
            } = maybe_con.await.map_err(TransportError::other)?;

            let con = ConItem {
                con,
                channel_limit: Arc::new(tokio::sync::Semaphore::new(channel_limit as usize)),
            };

            // if we are making an outgoing connection
            // we also need to make an initial channel
            let out = if with_channel {
                // acquire our first channel limit permit
                // this should always resolve immediately, given our
                // semaphore is new
                let permit = con.channel_limit.clone().acquire_owned().await;
                let (bi_send, bi_recv) = con.con.open_bi().await.map_err(TransportError::other)?;
                Some(tx_bi_chan(bi_send, bi_recv, permit))
            } else {
                None
            };

            // Construct our url from the low-level data
            let url = url2!("{}://{}", crate::SCHEME, con.con.remote_address());
            tracing::debug!("QUIC handle connection: {}", url);

            // clone our channel limiter semaphore
            // so we pause awaiting incoming streams for a permit
            let channel_limit = con.channel_limit.clone();

            // pass the connection off to our actor
            i_s.set_connection(url.clone(), con).await?;

            // pass any incoming channels off to our actor
            let url_clone = url.clone();
            metric_task(async move {
                loop {
                    // acquire a new channel_limit permit before accepting
                    // a new incoming channel
                    let permit = channel_limit.clone().acquire_owned().await;
                    match bi_streams.next().await {
                        Some(Err(e)) => {
                            tracing::warn!("incoming stream close: {:?}", e);
                        }
                        Some(Ok((bi_send, bi_recv))) => {
                            let (write, read) = tx_bi_chan(bi_send, bi_recv, permit);
                            let res = incoming_channel_sender
                                .send(TransportEvent::IncomingChannel(
                                    url_clone.clone(),
                                    write,
                                    read,
                                ))
                                .await;

                            if res.is_err() {
                                break;
                            }
                        }
                        _ => (),
                    }
                }
                <Result<(), ()>>::Ok(())
            });

            Ok(out.map(move |(write, read)| (url, write, read)))
        }
        .boxed()
        .into())
    }

    fn handle_set_connection(&mut self, url: Url2, con: ConItem) -> ListenerInnerHandlerResult<()> {
        self.connections.insert(url, con);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_drop_connection(&mut self, url: Url2) -> ListenerInnerHandlerResult<()> {
        self.connections.remove(&url);
        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<TransportListener> for TransportListenerQuic {}

impl TransportListenerHandler for TransportListenerQuic {
    fn handle_debug(&mut self) -> TransportListenerHandlerResult<serde_json::Value> {
        let url = self.bound_url.clone();
        let connections = self.connections.keys().cloned().collect::<Vec<_>>();
        Ok(async move {
            Ok(serde_json::json! {{
                "url": url,
                "connection_count": connections.len(),
            }})
        }
        .boxed()
        .into())
    }

    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<Url2> {
        let out = self.bound_url.clone();
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_create_channel(
        &mut self,
        url: Url2,
    ) -> TransportListenerHandlerResult<(Url2, TransportChannelWrite, TransportChannelRead)> {
        // if we already have an open connection to the remote end,
        // just directly try to open the bi-stream channel.
        let maybe_con = self.connections.get(&url).cloned();

        let i_s = self.internal_sender.clone();
        Ok(async move {
            // if we already had a connection and the bi-stream
            // channel is successfully opened, return early using that
            if let Some(con) = maybe_con {
                // await our channel limit semaphore before opening the channel
                let permit = con.channel_limit.acquire_owned().await;
                match con.con.open_bi().await {
                    Ok((bi_send, bi_recv)) => {
                        let (write, read) = tx_bi_chan(bi_send, bi_recv, permit);
                        return Ok((url, write, read));
                    }
                    Err(_) => {
                        // otherwise, we should drop any existing channel
                        // we have... it no longer works for us
                        i_s.drop_connection(url.clone()).await?;
                    }
                }
            }

            // we did not successfully use an existing connection.
            // instead, try establishing a new one with a new channel.
            let addr = crate::url_to_addr(&url, crate::SCHEME).await?;
            let maybe_con = i_s.raw_connect(addr).await?;
            let (url, write, read) = i_s.take_connecting(maybe_con, true).await?.unwrap();

            Ok((url, write, read))
        }
        .boxed()
        .into())
    }
}

/// Spawn a new QUIC TransportListenerSender.
pub async fn spawn_transport_listener_quic(
    config: ConfigListenerQuic,
    tuning_params: Arc<KitsuneP2pTuningParams>,
) -> TransportListenerResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportEventReceiver,
)> {
    let bind_to = config
        .bind_to
        .unwrap_or_else(|| url2::url2!("kitsune-quic://0.0.0.0:0"));
    let server_config = danger::configure_server(config.tls, tuning_params.clone())
        .await
        .map_err(|e| TransportError::from(format!("cert error: {:?}", e)))?;
    let mut builder = quinn::Endpoint::builder();
    builder.listen(server_config);
    builder.default_client_config(danger::configure_client(tuning_params.clone()));
    let (quinn_endpoint, incoming) = builder
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::other)?;

    let concurrent_recv = tuning_params.concurrent_recv_buffer as usize;
    let (incoming_channel_sender, receiver) = futures::channel::mpsc::channel(concurrent_recv);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder.channel_factory().create_channel().await?;

    let sender = builder.channel_factory().create_channel().await?;

    let i_s = internal_sender.clone();
    metric_task(async move {
        incoming
            .for_each_concurrent(concurrent_recv, |maybe_con| async {
                let res: TransportResult<()> = async {
                    i_s.take_connecting(maybe_con, false).await?;
                    Ok(())
                }
                .await;
                if let Err(err) = res {
                    ghost_actor::dependencies::tracing::error!(?err);
                }
            })
            .await;

        // Our incoming connections ended,
        // this also indicates we cannot establish outgoing connections.
        // I.e., we need to shut down.
        i_s.ghost_actor_shutdown().await?;

        TransportResult::Ok(())
    });

    let mut bound_url = url2!(
        "{}://{}",
        crate::SCHEME,
        quinn_endpoint.local_addr().map_err(TransportError::other)?,
    );
    if let Some(override_host) = &config.override_host {
        bound_url.set_host(Some(override_host)).unwrap();
    } else if let Some(host) = bound_url.host_str() {
        if host == "0.0.0.0" {
            for iface in if_addrs::get_if_addrs().map_err(TransportError::other)? {
                // super naive - just picking the first v4 that is not 127.0.0.1
                let addr = iface.addr.ip();
                if let std::net::IpAddr::V4(addr) = addr {
                    if addr != std::net::Ipv4Addr::from([127, 0, 0, 1]) {
                        bound_url
                            .set_host(Some(&iface.addr.ip().to_string()))
                            .unwrap();
                        break;
                    }
                }
            }
        }
    }

    let actor = TransportListenerQuic {
        internal_sender,
        incoming_channel_sender,
        bound_url,
        quinn_endpoint,
        connections: HashMap::new(),
        tuning_params,
    };

    metric_task(builder.spawn(actor));

    Ok((sender, receiver))
}

// TODO - modernize all this taking hints from TLS code in proxy crate.
mod danger {
    use kitsune_p2p_types::config::KitsuneP2pTuningParams;
    use kitsune_p2p_types::transport::TransportError;
    use kitsune_p2p_types::transport::TransportResult;
    use quinn::Certificate;
    use quinn::CertificateChain;
    use quinn::ClientConfig;
    use quinn::ClientConfigBuilder;
    use quinn::PrivateKey;
    use quinn::ServerConfig;
    use quinn::ServerConfigBuilder;
    use std::sync::Arc;

    fn transport_config(tuning_params: Arc<KitsuneP2pTuningParams>) -> Arc<quinn::TransportConfig> {
        let mut transport = quinn::TransportConfig::default();

        const EXPECTED_RTT: u64 = 100; // ms
        const MAX_STREAM_BANDWIDTH: u64 = 12500 * 1000; // bytes/s
        const STREAM_RWND: u64 = MAX_STREAM_BANDWIDTH / 1000 * EXPECTED_RTT;

        let w_mult = tuning_params.quic_window_multiplier as u64;

        transport.stream_window_bidi(32 * w_mult);
        transport.stream_receive_window(STREAM_RWND * w_mult);
        transport.receive_window(8 * STREAM_RWND * w_mult);
        transport.send_window(8 * STREAM_RWND * w_mult);

        let c_mult = tuning_params.quic_crypto_buffer_multiplier as usize;
        transport.crypto_buffer_size(16 * 1024 * c_mult);

        // We don't use uni streams in kitsune - only bidi streams
        transport.stream_window_uni(0);

        // We don't use "Application" datagrams in kitsune -
        // only bidi streams.
        transport.datagram_receive_buffer_size(None);
        transport.datagram_send_buffer_size(0);

        // Disable spin bit - we'd like the extra privacy
        // any metrics we implement will be opt-in self reporting
        transport.allow_spin(false);

        // see also `keep_alive_interval`.
        // right now keep_alive_interval is None,
        // so connections will idle timeout after this interval.
        let timeout = tuning_params.quic_max_idle_timeout_ms as u64;
        transport
            .max_idle_timeout(Some(std::time::Duration::from_millis(timeout)))
            .unwrap();

        Arc::new(transport)
    }

    #[allow(dead_code)]
    pub(crate) async fn configure_server(
        cert: Option<(
            lair_keystore_api::actor::Cert,
            lair_keystore_api::actor::CertPrivKey,
        )>,
        tuning_params: Arc<KitsuneP2pTuningParams>,
    ) -> TransportResult<ServerConfig> {
        let (cert, cert_priv) = match cert {
            Some(r) => r,
            None => {
                let mut options = lair_keystore_api::actor::TlsCertOptions::default();
                options.alg = lair_keystore_api::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
                let cert = lair_keystore_api::internal::tls::tls_cert_self_signed_new_from_entropy(
                    options,
                )
                .await
                .map_err(TransportError::other)?;
                (cert.cert_der, cert.priv_key_der)
            }
        };

        let tcert = Certificate::from_der(&cert).map_err(TransportError::other)?;
        let tcert_priv = PrivateKey::from_der(&cert_priv).map_err(TransportError::other)?;

        let server_config = ServerConfig::default();
        let mut cfg_builder = ServerConfigBuilder::new(server_config);
        cfg_builder
            .certificate(CertificateChain::from_certs(vec![tcert]), tcert_priv)
            .map_err(TransportError::other)?;

        let mut cfg = cfg_builder.build();

        cfg.transport = transport_config(tuning_params);
        Ok(cfg)
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

    pub(crate) fn configure_client(tuning_params: Arc<KitsuneP2pTuningParams>) -> ClientConfig {
        let mut cfg = ClientConfigBuilder::default().build();
        let tls_cfg: &mut rustls::ClientConfig = Arc::get_mut(&mut cfg.crypto).unwrap();
        // this is only available when compiled with "dangerous_configuration" feature
        tls_cfg
            .dangerous()
            .set_certificate_verifier(SkipServerVerification::new());

        cfg.transport = transport_config(tuning_params);
        cfg
    }
}
