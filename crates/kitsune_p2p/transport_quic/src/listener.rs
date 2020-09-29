use futures::{future::FutureExt, sink::SinkExt, stream::StreamExt};
use kitsune_p2p_types::{
    dependencies::{ghost_actor, ghost_actor::GhostControlSender, url2::*},
    transport::*,
};
use std::{collections::HashMap, net::SocketAddr};

/// Convert quinn async read/write streams into Vec<u8> senders / receivers.
fn tx_bi_chan(
    mut bi_send: quinn::SendStream,
    mut bi_recv: quinn::RecvStream,
) -> (TransportChannelWrite, TransportChannelRead) {
    let (write_send, mut write_recv) = futures::channel::mpsc::channel::<Vec<u8>>(10);
    let write_send = write_send.sink_map_err(TransportError::other);
    tokio::task::spawn(async move {
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
    tokio::task::spawn(async move {
        let mut buf = [0_u8; 4096];
        while let Some(read) = bi_recv
            .read(&mut buf)
            .await
            .map_err(TransportError::other)?
        {
            if read == 0 {
                continue;
            }
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

/// QUIC implementation of kitsune TransportListener actor.
struct TransportListenerQuic {
    /// internal api logic
    internal_sender: ghost_actor::GhostSender<ListenerInner>,
    /// incoming channel send to our owner
    incoming_channel_sender: TransportIncomingChannelSender,
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

impl ghost_actor::GhostHandler<ListenerInner> for TransportListenerQuic {}

impl ListenerInnerHandler for TransportListenerQuic {
    fn handle_raw_connect(
        &mut self,
        addr: SocketAddr,
    ) -> ListenerInnerHandlerResult<quinn::Connecting> {
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
            let quinn::NewConnection {
                connection: con,
                mut bi_streams,
                ..
            } = maybe_con.await.map_err(TransportError::other)?;
            let out = if with_channel {
                let (bi_send, bi_recv) = con.open_bi().await.map_err(TransportError::other)?;
                Some(tx_bi_chan(bi_send, bi_recv))
            } else {
                None
            };
            let url = url2!("{}://{}", crate::SCHEME, con.remote_address());
            i_s.set_connection(url.clone(), con).await?;
            let url_clone = url.clone();
            tokio::task::spawn(async move {
                while let Some(Ok((bi_send, bi_recv))) = bi_streams.next().await {
                    let (write, read) = tx_bi_chan(bi_send, bi_recv);
                    if let Err(_) = incoming_channel_sender
                        .send((url_clone.clone(), write, read))
                        .await
                    {
                        break;
                    }
                }
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

    fn handle_create_channel(
        &mut self,
        url: Url2,
    ) -> TransportListenerHandlerResult<(Url2, TransportChannelWrite, TransportChannelRead)> {
        let maybe_bi = self.connections.get(&url).map(|con| con.open_bi());

        let i_s = self.internal_sender.clone();
        Ok(async move {
            if let Some(maybe_bi) = maybe_bi {
                match maybe_bi.await {
                    Ok((bi_send, bi_recv)) => {
                        let (write, read) = tx_bi_chan(bi_send, bi_recv);
                        return Ok((url, write, read));
                    }
                    Err(_) => {
                        i_s.drop_connection(url.clone()).await?;
                    }
                }
            }
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
    bind_to: Url2,
    cert: Option<(
        lair_keystore_api::actor::Cert,
        lair_keystore_api::actor::CertPrivKey,
    )>,
) -> TransportListenerResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportIncomingChannelReceiver,
)> {
    let server_config = danger::configure_server(cert)
        .await
        .map_err(|e| TransportError::from(format!("cert error: {:?}", e)))?;
    let mut builder = quinn::Endpoint::builder();
    builder.listen(server_config);
    builder.default_client_config(danger::configure_client());
    let (quinn_endpoint, incoming) = builder
        .bind(&crate::url_to_addr(&bind_to, crate::SCHEME).await?)
        .map_err(TransportError::other)?;

    let (incoming_channel_sender, receiver) = futures::channel::mpsc::channel(10);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let internal_sender = builder.channel_factory().create_channel().await?;

    let sender = builder.channel_factory().create_channel().await?;

    let i_s = internal_sender.clone();
    tokio::task::spawn(async move {
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

    let actor = TransportListenerQuic {
        internal_sender,
        incoming_channel_sender,
        quinn_endpoint,
        connections: HashMap::new(),
    };

    tokio::task::spawn(builder.spawn(actor));

    Ok((sender, receiver))
}

mod danger {
    use kitsune_p2p_types::transport::{TransportError, TransportResult};
    use quinn::{
        Certificate, CertificateChain, ClientConfig, ClientConfigBuilder, PrivateKey, ServerConfig,
        ServerConfigBuilder, TransportConfig,
    };
    use std::sync::Arc;

    #[allow(dead_code)]
    pub(crate) async fn configure_server(
        cert: Option<(
            lair_keystore_api::actor::Cert,
            lair_keystore_api::actor::CertPrivKey,
        )>,
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

        let mut transport_config = TransportConfig::default();
        transport_config.stream_window_uni(0);
        let mut server_config = ServerConfig::default();
        server_config.transport = Arc::new(transport_config);
        let mut cfg_builder = ServerConfigBuilder::new(server_config);
        cfg_builder
            .certificate(CertificateChain::from_certs(vec![tcert]), tcert_priv)
            .map_err(TransportError::other)?;

        Ok(cfg_builder.build())
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
