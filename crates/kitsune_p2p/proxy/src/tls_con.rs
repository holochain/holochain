use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};
use rustls::Session;
use std::io::{Read, Write};

ghost_actor::ghost_chan! {
    pub(crate) chan TlsConnection<TransportError> {
        fn req_proxy() -> Arc<ProxyUrl>;
        fn chan_new(proxy_url: Arc<ProxyUrl>) -> ChannelId;
        fn chan_send(channel_id: ChannelId, channel_data: ChannelData) -> ();
        fn chan_drop(channel_id: ChannelId) -> ();
    }
}

pub(crate) type TlsConnectionReceiver = futures::channel::mpsc::Receiver<TlsConnection>;

pub(crate) async fn spawn_tls_connection(
    sub_sender: ghost_actor::GhostSender<TransportConnection>,
    sub_receiver: TransportConnectionEventReceiver,
    tls: TlsConfig,
    tls_server_config: Arc<rustls::ServerConfig>,
    tls_client_config: Arc<rustls::ClientConfig>,
) -> TransportResult<(
    ghost_actor::GhostSender<TlsConnection>,
    TlsConnectionReceiver,
)> {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TlsConnection>()
        .await?;

    builder
        .channel_factory()
        .attach_receiver(sub_receiver)
        .await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(builder.spawn(InnerTls::new(
        tls,
        tls_server_config,
        tls_client_config,
        sub_sender,
        evt_send,
    )?));

    Ok((sender, evt_recv))
}

#[allow(dead_code)]
pub(crate) struct InnerTls {
    tls: TlsConfig,
    tls_server_config: Arc<rustls::ServerConfig>,
    tls_client_config: Arc<rustls::ClientConfig>,
    sub_sender: ghost_actor::GhostSender<TransportConnection>,
    evt_send: futures::channel::mpsc::Sender<TlsConnection>,
}

impl InnerTls {
    pub fn new(
        tls: TlsConfig,
        tls_server_config: Arc<rustls::ServerConfig>,
        tls_client_config: Arc<rustls::ClientConfig>,
        sub_sender: ghost_actor::GhostSender<TransportConnection>,
        evt_send: futures::channel::mpsc::Sender<TlsConnection>,
    ) -> TransportResult<Self> {
        Ok(Self {
            tls,
            tls_server_config,
            tls_client_config,
            sub_sender,
            evt_send,
        })
    }

    fn request(&self, data: ProxyWire) -> TlsConnectionHandlerResult<ProxyWire> {
        let nr = webpki::DNSNameRef::try_from_ascii_str("stub.stub").unwrap();
        let mut cli = rustls::ClientSession::new(&self.tls_client_config, nr);
        let fut = self.sub_sender.create_channel();
        Ok(async move {
            let (mut write, mut read) = fut.await?;

            let data = data.encode()?;
            cli.write_all(&data).map_err(TransportError::other)?;

            let mut buf = [0_u8; 4096];
            let mut in_pre = std::io::Cursor::new(Vec::new());
            let mut in_post = Vec::new();
            loop {
                if cli.wants_write() {
                    let mut data = Vec::new();
                    cli.write_tls(&mut data).map_err(TransportError::other)?;
                    write.send(data).await?;
                }

                if !cli.wants_read() {
                    tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                    continue;
                }

                match read.next().await {
                    None => return Err("failed to get response".into()),
                    Some(data) => {
                        in_pre.get_mut().extend_from_slice(&data);
                        cli.read_tls(&mut in_pre).map_err(TransportError::other)?;
                        cli.process_new_packets().map_err(TransportError::other)?;
                        let size = cli.read(&mut buf).map_err(TransportError::other)?;
                        in_post.extend_from_slice(&buf[..size]);

                        if let Ok(proxy_wire) = ProxyWire::decode(&in_post) {
                            return Ok(proxy_wire);
                        }
                    }
                }
            }
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostControlHandler for InnerTls {
    fn handle_ghost_actor_shutdown(self) -> MustBoxFuture<'static, ()> {
        async move {
            let _ = self.sub_sender.ghost_actor_shutdown().await;
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<TlsConnection> for InnerTls {}

impl TlsConnectionHandler for InnerTls {
    fn handle_req_proxy(&mut self) -> TlsConnectionHandlerResult<Arc<ProxyUrl>> {
        let data = ProxyWire::req_proxy(MsgId::next());
        let fut = self.request(data)?;
        Ok(async move {
            let data = fut.await?;
            match data {
                ProxyWire::ReqProxyOk(ok) => Ok(Arc::new(ok.1.into())),
                ProxyWire::ReqProxyErr(err) => Err(err.1.into()),
                _ => Err("bad response".into()),
            }
        }
        .boxed()
        .into())
    }

    fn handle_chan_new(
        &mut self,
        proxy_url: Arc<ProxyUrl>,
    ) -> TlsConnectionHandlerResult<ChannelId> {
        let data = ProxyWire::chan_new(MsgId::next(), (&*proxy_url).into());
        let fut = self.request(data)?;
        Ok(async move {
            let data = fut.await?;
            match data {
                ProxyWire::ChanNewOk(ok) => Ok(ok.1),
                ProxyWire::ChanNewErr(err) => Err(err.1.into()),
                _ => Err("bad response".into()),
            }
        }
        .boxed()
        .into())
    }

    fn handle_chan_send(
        &mut self,
        _channel_id: ChannelId,
        _channel_data: ChannelData,
    ) -> TlsConnectionHandlerResult<()> {
        unimplemented!()
    }

    fn handle_chan_drop(&mut self, _channel_id: ChannelId) -> TlsConnectionHandlerResult<()> {
        unimplemented!()
    }
}

impl ghost_actor::GhostHandler<TransportConnectionEvent> for InnerTls {}

impl TransportConnectionEventHandler for InnerTls {
    fn handle_incoming_channel(
        &mut self,
        _url: url2::Url2,
        _send: TransportChannelWrite,
        _recv: TransportChannelRead,
    ) -> TransportConnectionEventHandlerResult<()> {
        let mut _srv = rustls::ServerSession::new(&self.tls_server_config);
        unimplemented!()
    }
}
