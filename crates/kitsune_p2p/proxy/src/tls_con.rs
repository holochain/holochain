use crate::*;

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
        unimplemented!()
    }

    fn handle_chan_new(
        &mut self,
        _proxy_url: Arc<ProxyUrl>,
    ) -> TlsConnectionHandlerResult<ChannelId> {
        unimplemented!()
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
        unimplemented!()
    }
}
