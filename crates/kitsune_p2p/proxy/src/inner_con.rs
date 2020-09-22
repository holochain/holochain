use crate::*;

pub(crate) async fn spawn_kitsune_proxy_connection(
    sub_sender: ghost_actor::GhostSender<TlsConnection>,
    sub_receiver: TlsConnectionReceiver,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportConnection>,
    TransportConnectionEventReceiver,
)> {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TransportConnection>()
        .await?;

    builder
        .channel_factory()
        .attach_receiver(sub_receiver)
        .await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(builder.spawn(InnerCon::new(sub_sender, evt_send)?));

    Ok((sender, evt_recv))
}

pub(crate) struct InnerCon {
    sub_sender: ghost_actor::GhostSender<TlsConnection>,
    _evt_send: futures::channel::mpsc::Sender<TransportConnectionEvent>,
}

impl InnerCon {
    pub fn new(
        sub_sender: ghost_actor::GhostSender<TlsConnection>,
        evt_send: futures::channel::mpsc::Sender<TransportConnectionEvent>,
    ) -> TransportResult<Self> {
        Ok(Self {
            sub_sender,
            _evt_send: evt_send,
        })
    }
}

impl ghost_actor::GhostControlHandler for InnerCon {
    fn handle_ghost_actor_shutdown(self) -> MustBoxFuture<'static, ()> {
        async move {
            let _ = self.sub_sender.ghost_actor_shutdown().await;
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<TransportConnection> for InnerCon {}

impl TransportConnectionHandler for InnerCon {
    fn handle_remote_url(&mut self) -> TransportConnectionHandlerResult<url2::Url2> {
        unimplemented!()
    }

    fn handle_create_channel(
        &mut self,
    ) -> TransportConnectionHandlerResult<(TransportChannelWrite, TransportChannelRead)> {
        unimplemented!()
    }
}

impl ghost_actor::GhostHandler<TlsConnection> for InnerCon {}

impl TlsConnectionHandler for InnerCon {
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
