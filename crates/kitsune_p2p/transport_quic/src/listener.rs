use crate::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::dependencies::ghost_actor::GhostControlSender;
use kitsune_p2p_types::dependencies::serde_json;
use kitsune_p2p_types::dependencies::url2;
use kitsune_p2p_types::transport::*;
use std::collections::HashMap;
use std::net::SocketAddr;

/// Convert quinn async read/write streams into Vec<u8> senders / receivers.
/// Quic bi-streams are Async Read/Write - But the kitsune transport api
/// uses Vec<u8> Streams / Sinks - This code translates into that.
fn tx_bi_chan(
    mut bi_send: quinn::SendStream,
    mut bi_recv: quinn::RecvStream,
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
    let write_send: TransportChannelWrite = Box::new(write_send);
    let read_recv: TransportChannelRead = Box::new(read_recv);
    (write_send, read_recv)
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
    connections: HashMap<Url2, quinn::Connection>,
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
                con.close(0_u8.into(), b"");
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
            con: quinn::Connection,
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

            // if we are making an outgoing connection
            // we also need to make an initial channel
            let out = if with_channel {
                let (bi_send, bi_recv) = con.open_bi().await.map_err(TransportError::other)?;
                Some(tx_bi_chan(bi_send, bi_recv))
            } else {
                None
            };

            // Construct our url from the low-level data
            let url = url2!("{}://{}", crate::SCHEME, con.remote_address());
            tracing::debug!("QUIC handle connection: {}", url);

            // pass the connection off to our actor
            i_s.set_connection(url.clone(), con).await?;

            // pass any incoming channels off to our actor
            let url_clone = url.clone();
            metric_task(async move {
                while let Some(Ok((bi_send, bi_recv))) = bi_streams.next().await {
                    let (write, read) = tx_bi_chan(bi_send, bi_recv);
                    if incoming_channel_sender
                        .send(TransportEvent::IncomingChannel(
                            url_clone.clone(),
                            write,
                            read,
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                <Result<(), ()>>::Ok(())
            });

            Ok(out.map(move |(write, read)| (url, write, read)))
        }
        .boxed()
        .into())
    }

    fn handle_set_connection(
        &mut self,
        url: Url2,
        con: quinn::Connection,
    ) -> ListenerInnerHandlerResult<()> {
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
        let maybe_bi = self.connections.get(&url).map(|con| con.open_bi());

        let i_s = self.internal_sender.clone();
        Ok(async move {
            // if we already had a connection and the bi-stream
            // channel is successfully opened, return early using that
            if let Some(maybe_bi) = maybe_bi {
                match maybe_bi.await {
                    Ok((bi_send, bi_recv)) => {
                        let (write, read) = tx_bi_chan(bi_send, bi_recv);
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
) -> TransportListenerResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportEventReceiver,
)> {
    let bind_to = config
        .bind_to
        .unwrap_or_else(|| url2::url2!("kitsune-quic://0.0.0.0:0"));
    let server_config = danger::configure_server(config.tls)
        .await
        .map_err(|e| TransportError::from(format!("cert error: {:?}", e)))?;
    let mut builder = quinn::Endpoint::builder();
    builder.listen(server_config);
    builder.default_client_config(danger::configure_client()?);
    let (quinn_endpoint, incoming) = builder
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::other)?;

    let (incoming_channel_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder.channel_factory().create_channel().await?;

    let sender = builder.channel_factory().create_channel().await?;

    let i_s = internal_sender.clone();
    metric_task(async move {
        incoming
            .for_each_concurrent(10, |maybe_con| async {
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
    };

    metric_task(builder.spawn(actor));

    Ok((sender, receiver))
}

// TODO - modernize all this taking hints from TLS code in proxy crate.
mod danger {
    use crate::lair_keystore_api_0_0;
    use kitsune_p2p_types::transport::TransportError;
    use kitsune_p2p_types::transport::TransportResult;
    use once_cell::sync::Lazy;
    use quinn::Certificate;
    use quinn::CertificateChain;
    use quinn::ClientConfig;
    use quinn::ClientConfigBuilder;
    use quinn::PrivateKey;
    use quinn::ServerConfig;
    use quinn::ServerConfigBuilder;
    use quinn::TransportConfig;
    use std::sync::Arc;

    // TODO: make this a prop error type
    static TRANSPORT: Lazy<Result<Arc<quinn::TransportConfig>, String>> = Lazy::new(|| {
        let mut transport = quinn::TransportConfig::default();

        // We don't use uni streams in kitsune - only bidi streams
        transport
            .max_concurrent_uni_streams(0)
            .map_err(|e| e.to_string())?;

        // We don't use "Application" datagrams in kitsune -
        // only bidi streams.
        transport.datagram_receive_buffer_size(None);

        // Disable spin bit - we'd like the extra privacy
        // any metrics we implement will be opt-in self reporting
        transport.allow_spin(false);

        // see also `keep_alive_interval`.
        // right now keep_alive_interval is None,
        // so connections will idle timeout after 20 seconds.
        transport
            .max_idle_timeout(Some(std::time::Duration::from_millis(30_000)))
            .unwrap();

        Ok(Arc::new(transport))
    });

    #[allow(dead_code)]
    pub(crate) async fn configure_server(
        cert: Option<(
            lair_keystore_api_0_0::actor::Cert,
            lair_keystore_api_0_0::actor::CertPrivKey,
        )>,
    ) -> TransportResult<ServerConfig> {
        let (cert, cert_priv) = match cert {
            Some(r) => r,
            None => {
                let mut options = lair_keystore_api_0_0::actor::TlsCertOptions::default();
                options.alg = lair_keystore_api_0_0::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
                let cert =
                    lair_keystore_api_0_0::internal::tls::tls_cert_self_signed_new_from_entropy(
                        options,
                    )
                    .await
                    .map_err(TransportError::other)?;
                (cert.cert_der, cert.priv_key_der)
            }
        };

        let tcert = Certificate::from_der(&cert).map_err(TransportError::other)?;
        let tcert_priv = PrivateKey::from_der(&cert_priv).map_err(TransportError::other)?;

        let mut transport_config = TransportConfig::default();
        transport_config
            .max_concurrent_uni_streams(0)
            .map_err(TransportError::other)?;
        let mut server_config = ServerConfig::default();
        server_config.transport = Arc::new(transport_config);
        let mut cfg_builder = ServerConfigBuilder::new(server_config);
        cfg_builder
            .certificate(CertificateChain::from_certs(vec![tcert]), tcert_priv)
            .map_err(TransportError::other)?;

        let mut cfg = cfg_builder.build();

        cfg.transport = TRANSPORT.clone()?;
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

    pub(crate) fn configure_client() -> Result<ClientConfig, String> {
        let mut cfg = ClientConfigBuilder::default().build();
        let tls_cfg: &mut rustls::ClientConfig = Arc::get_mut(&mut cfg.crypto).unwrap();
        // this is only available when compiled with "dangerous_configuration" feature
        tls_cfg
            .dangerous()
            .set_certificate_verifier(SkipServerVerification::new());

        cfg.transport = TRANSPORT.clone()?;
        Ok(cfg)
    }
}
