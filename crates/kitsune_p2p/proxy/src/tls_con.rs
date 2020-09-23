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

pub(crate) struct InnerTls {
    _tls: TlsConfig,
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
            _tls: tls,
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
                            write.close().await?;
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
        mut write: TransportChannelWrite,
        mut read: TransportChannelRead,
    ) -> TransportConnectionEventHandlerResult<()> {
        let mut srv = rustls::ServerSession::new(&self.tls_server_config);
        let evt_send = self.evt_send.clone();
        Ok(async move {
            let mut buf = [0_u8; 4096];
            let mut in_pre = std::io::Cursor::new(Vec::new());
            let mut in_post = Vec::new();
            loop {
                if srv.wants_write() {
                    let mut data = Vec::new();
                    srv.write_tls(&mut data).map_err(TransportError::other)?;
                    write.send(data).await?;
                }

                if !srv.wants_read() {
                    tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                }

                match read.next().await {
                    None => return Err("failed to retrieve request".into()),
                    Some(data) => {
                        in_pre.get_mut().extend_from_slice(&data);
                        srv.read_tls(&mut in_pre).map_err(TransportError::other)?;
                        srv.process_new_packets().map_err(TransportError::other)?;
                        let size = srv.read(&mut buf).map_err(TransportError::other)?;
                        in_post.extend_from_slice(&buf[..size]);
                        if let Ok(proxy_wire) = ProxyWire::decode(&in_post) {
                            let res = match proxy_wire {
                                ProxyWire::ReqProxy(_) => {
                                    evt_send.req_proxy().await.map(|proxy_url| {
                                        ProxyWire::req_proxy_ok(MsgId::next(), (&*proxy_url).into())
                                    })
                                }
                                _ => return Ok(()),
                            }?
                            .encode()?;
                            srv.write_all(&res).map_err(TransportError::other)?;

                            loop {
                                if srv.wants_write() {
                                    let mut data = Vec::new();
                                    srv.write_tls(&mut data).map_err(TransportError::other)?;
                                    write.send(data).await?;
                                } else {
                                    write.close().await?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        .boxed()
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tls_connection() -> TransportResult<()> {
        let tls = TlsConfig::new_ephemeral().await?;
        let (tls_server_config, tls_client_config) = inner_listen::gen_tls_configs(&tls)?;

        let (bind1, evt1) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
        let addr1 = bind1.bound_url().await?;
        println!("got bind1 = {}", addr1);

        let (bind2, evt2) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
        let addr2 = bind2.bound_url().await?;
        println!("got bind2 = {}", addr2);

        let track_con = move |mut recv: TlsConnectionReceiver| {
            tokio::task::spawn(async move {
                while let Some(_msg) = recv.next().await {
                }
            });
        };

        let t_tls = tls.clone();
        let t_srv = tls_server_config.clone();
        let t_cli = tls_client_config.clone();
        let track_listen = move |mut recv: TransportListenerEventReceiver| {
            let t_tls = t_tls.clone();
            let t_srv = t_srv.clone();
            let t_cli = t_cli.clone();
            tokio::task::spawn(async move {
                while let Some(msg) = recv.next().await {
                    match msg {
                        TransportListenerEvent::IncomingConnection {
                            respond, sender, receiver, ..
                        } => {
                            let (_sender, receiver) = spawn_tls_connection(
                                sender,
                                receiver,
                                t_tls.clone(),
                                t_srv.clone(),
                                t_cli.clone(),
                            ).await?;
                            track_con(receiver);
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                        }
                    }
                }
                TransportResult::Ok(())
            });
        };

        track_listen(evt1);
        track_listen(evt2);

        let (con1, con_evt1) = bind1.connect(addr2.clone()).await?;
        let (_con1, con_evt1) = spawn_tls_connection(
            con1,
            con_evt1,
            tls.clone(),
            tls_server_config.clone(),
            tls_client_config.clone(),
        ).await?;
        track_con(con_evt1);

        Ok(())
    }
}
