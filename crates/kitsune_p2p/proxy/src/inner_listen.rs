use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::dependencies::serde_json;
use std::collections::HashMap;

/// How often should NAT nodes refresh their proxy contract?
/// Note - ProxyTo entries will be expired at double this time.
const PROXY_KEEPALIVE_MS: u64 = 15000;
/// How much longer the proxy should wait to remove the contract
/// if no keep alive is received.
const KEEPALIVE_MULTIPLIER: u64 = 3;

/// Wrap a transport listener sender/receiver in kitsune proxy logic.
pub async fn spawn_kitsune_proxy_listener(
    proxy_config: Arc<ProxyConfig>,
    sub_sender: ghost_actor::GhostSender<TransportListener>,
    mut sub_receiver: TransportEventReceiver,
) -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportEventReceiver,
)> {
    // sort out our proxy config
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

    // Configure our own proxy url based of connection details / tls cert.
    let this_url = sub_sender.bound_url().await?;
    let this_url = ProxyUrl::new(this_url.as_str(), tls.cert_digest.clone())?;

    // ghost acto builder!
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory
        .create_channel::<TransportListener>()
        .await?;

    let i_s = channel_factory.create_channel::<Internal>().await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    // spawn the actor
    metric_task!(builder.spawn(
        InnerListen::new(
            i_s.clone(),
            this_url,
            tls,
            accept_proxy_cb,
            sub_sender,
            evt_send,
        )
        .await?,
    ));

    // if we want to be proxied, we need to connect to our proxy
    // and manage that connection contract
    if let Some(proxy_url) = proxy_url {
        i_s.req_proxy(proxy_url.clone()).await?;

        // Set up a timer to refresh our proxy contract at keepalive interval
        let i_s_c = i_s.clone();
        metric_task!(async move {
            loop {
                tokio::time::delay_for(std::time::Duration::from_millis(PROXY_KEEPALIVE_MS)).await;

                if let Err(e) = i_s_c.req_proxy(proxy_url.clone()).await {
                    tracing::error!(msg = "renewing proxy failed", ?proxy_url, ?e);
                    // either we failed because the actor is already shutdown
                    // or the remote end rejected us.
                    // if it's the latter - shut down our ghost actor : )
                    if !i_s_c.ghost_actor_is_active() {
                        tracing::debug!("Ghost actor has closed so exiting keep alive");
                        break;
                    }
                } else {
                    tracing::info!("Proxy renewed for {:?}", proxy_url);
                }
            }
            tracing::error!("Keep alive closed");
        });
    }

    // handle incoming channels from our sub transport
    metric_task!(async move {
        while let Some(evt) = sub_receiver.next().await {
            match evt {
                TransportEvent::IncomingChannel(url, write, read) => {
                    // spawn so we can process incoming requests in parallel
                    let i_s = i_s.clone();
                    metric_task!(async move {
                        let _ = i_s.incoming_channel(url, write, read).await;
                    });
                }
            }
        }

        // Our incoming channels ended,
        // this also indicates we cannot establish outgoing connections.
        // I.e., we need to shut down.
        i_s.ghost_actor_shutdown().await?;

        TransportResult::Ok(())
    });

    Ok((sender, evt_recv))
}

#[derive(Debug)]
/// An item in our proxy_list - a client we have agreed to proxy for
struct ProxyTo {
    /// the low-level connection url
    base_connection_url: url2::Url2,

    /// when this proxy contract expires
    expires_at: std::time::Instant,
}

struct InnerListen {
    i_s: ghost_actor::GhostSender<Internal>,
    this_url: ProxyUrl,
    accept_proxy_cb: AcceptProxyCallback,
    sub_sender: ghost_actor::GhostSender<TransportListener>,
    evt_send: TransportEventSender,
    tls: TlsConfig,
    tls_server_config: Arc<rustls::ServerConfig>,
    tls_client_config: Arc<rustls::ClientConfig>,
    proxy_list: HashMap<ProxyUrl, ProxyTo>,
}

impl InnerListen {
    pub async fn new(
        i_s: ghost_actor::GhostSender<Internal>,
        this_url: ProxyUrl,
        tls: TlsConfig,
        accept_proxy_cb: AcceptProxyCallback,
        sub_sender: ghost_actor::GhostSender<TransportListener>,
        evt_send: TransportEventSender,
    ) -> TransportResult<Self> {
        tracing::info!(
            "{}: starting up with this_url: {}",
            this_url.short(),
            this_url
        );

        let (tls_server_config, tls_client_config) = gen_tls_configs(&tls)?;

        Ok(Self {
            i_s,
            this_url,
            accept_proxy_cb,
            sub_sender,
            evt_send,
            tls,
            tls_server_config,
            tls_client_config,
            proxy_list: HashMap::new(),
        })
    }
}

impl ghost_actor::GhostControlHandler for InnerListen {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        async move {
            let _ = self.sub_sender.ghost_actor_shutdown().await;
            self.evt_send.close_channel();
            tracing::warn!("proxy listener actor SHUTDOWN");
        }
        .boxed()
        .into()
    }
}

ghost_actor::ghost_chan! {
    chan Internal<TransportError> {
        fn incoming_channel(
            base_url: url2::Url2,
            write: TransportChannelWrite,
            read: TransportChannelRead,
        ) -> ();

        fn incoming_req_proxy(
            base_url: url2::Url2,
            _cert_digest: ChannelData,
            write: futures::channel::mpsc::Sender<ProxyWire>,
            read: futures::channel::mpsc::Receiver<ProxyWire>,
        ) -> ();

        fn incoming_chan_new(
            base_url: url2::Url2,
            dest_proxy_url: ProxyUrl,
            write: futures::channel::mpsc::Sender<ProxyWire>,
            read: futures::channel::mpsc::Receiver<ProxyWire>,
        ) -> ();

        fn create_low_level_channel(
            base_url: url2::Url2,
        ) -> (
            futures::channel::mpsc::Sender<ProxyWire>,
            futures::channel::mpsc::Receiver<ProxyWire>,
        );

        fn prune_bad_proxy_to(proxy_url: ProxyUrl) -> ();

        fn register_proxy_to(proxy_url: ProxyUrl, base_url: url2::Url2) -> ();

        fn req_proxy(proxy_url: ProxyUrl) -> ();
        fn set_proxy_url(proxy_url: ProxyUrl) -> ();
    }
}

impl ghost_actor::GhostHandler<Internal> for InnerListen {}

// If we're forwarding data to another channel,
// we need to forward all data read from a reader to a writer.
fn cross_join_channel_forward(
    mut write: futures::channel::mpsc::Sender<ProxyWire>,
    mut read: futures::channel::mpsc::Receiver<ProxyWire>,
) {
    metric_task!(async move {
        while let Some(msg) = read.next().await {
            // do we need to inspect these??
            // for now just forwarding everything
            write.send(msg).await.map_err(TransportError::other)?;
        }
        TransportResult::Ok(())
    });
}

impl InternalHandler for InnerListen {
    fn handle_incoming_channel(
        &mut self,
        base_url: url2::Url2,
        write: TransportChannelWrite,
        read: TransportChannelRead,
    ) -> InternalHandlerResult<()> {
        let short = self.this_url.short().to_string();
        tracing::debug!("{}: proxy, incoming channel: {}", short, base_url);
        let mut write = wire_write::wrap_wire_write(write);
        let mut read = wire_read::wrap_wire_read(read);
        let i_s = self.i_s.clone();
        Ok(async move {
            match read.next().await {
                Some(ProxyWire::ReqProxy(p)) => {
                    tracing::debug!("{}: req proxy: {:?}", short, p.cert_digest);
                    i_s.incoming_req_proxy(base_url, p.cert_digest, write, read)
                        .await?;
                }
                Some(ProxyWire::ChanNew(c)) => {
                    tracing::debug!("{}: chan new: {:?}", short, c.proxy_url);
                    i_s.incoming_chan_new(base_url, c.proxy_url.into(), write, read)
                        .await?;
                }
                e => {
                    tracing::error!("{}: invalid message {:?}", short, e);
                    write
                        .send(ProxyWire::failure(format!("invalid message {:?}", e)))
                        .await
                        .map_err(TransportError::other)?;
                }
            }
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_incoming_req_proxy(
        &mut self,
        base_url: url2::Url2,
        cert_digest: ChannelData,
        mut write: futures::channel::mpsc::Sender<ProxyWire>,
        _read: futures::channel::mpsc::Receiver<ProxyWire>,
    ) -> InternalHandlerResult<()> {
        tracing::info!(
            "{}: {} would like us to proxy them",
            self.this_url.short(),
            base_url
        );
        let accept_proxy_cb = self.accept_proxy_cb.clone();
        let proxy_url = ProxyUrl::new(self.this_url.as_base().as_str(), cert_digest.0.into())?;
        let i_s = self.i_s.clone();
        Ok(async move {
            if !accept_proxy_cb(vec![32].into()).await {
                write
                    .send(ProxyWire::failure("Proxy Request Rejected".into()))
                    .await
                    .map_err(TransportError::other)?;
                return Ok(());
            }

            i_s.register_proxy_to(proxy_url.clone(), base_url).await?;

            write
                .send(ProxyWire::req_proxy_ok(proxy_url.into()))
                .await
                .map_err(TransportError::other)?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_incoming_chan_new(
        &mut self,
        base_url: url2::Url2,
        dest_proxy_url: ProxyUrl,
        mut write: futures::channel::mpsc::Sender<ProxyWire>,
        read: futures::channel::mpsc::Receiver<ProxyWire>,
    ) -> InternalHandlerResult<()> {
        let short = self.this_url.short().to_string();

        // just prune the proxy_list every time before we check for now
        let now = std::time::Instant::now();
        self.proxy_list.retain(|_, p| p.expires_at >= now);

        // first check to see if we should proxy this
        // to a client we are servicing.
        let proxy_to = if let Some(proxy_to) = self.proxy_list.get(&dest_proxy_url) {
            Some(proxy_to.base_connection_url.clone())
        } else {
            None
        };

        // if we're not proxying for a client,
        // check to see if our owner is the destination.
        if proxy_to.is_none() && dest_proxy_url.as_base() == self.this_url.as_base() {
            if dest_proxy_url.as_full() == self.this_url.as_full() {
                tracing::debug!("{}: chan new to self, hooking connection", short);

                // Hey! They're trying to talk to us!
                // Let's connect them to our owner.
                tls_srv::spawn_tls_server(
                    short,
                    base_url,
                    self.tls_server_config.clone(),
                    self.evt_send.clone(),
                    write,
                    read,
                );
            } else {
                tracing::warn!("Dropping message for {}", dest_proxy_url.as_full_str());
                return Ok(async move {
                    write
                        .send(ProxyWire::failure(format!(
                            "Dropped message to {}",
                            dest_proxy_url.as_full_str()
                        )))
                        .await
                        .map_err(TransportError::other)?;
                    Ok(())
                }
                .boxed()
                .into());
            }

            return Ok(async move { Ok(()) }.boxed().into());
        }

        // if we are proxying - forward to another channel
        // this is sensitive to url naming...
        // we're assuming our sub-transport is holding open a connection
        // and the channel create will re-use that.
        // If it is not, it will try to create a new connection that may fail.
        let fut = match proxy_to {
            None => {
                tracing::warn!("Dropping message for {}", dest_proxy_url.as_full_str());
                return Ok(async move {
                    write
                        .send(ProxyWire::failure(format!(
                            "Dropped message to {}",
                            dest_proxy_url.as_full_str()
                        )))
                        .await
                        .map_err(TransportError::other)?;
                    Ok(())
                }
                .boxed()
                .into());
            }
            Some(proxy_to) => self.i_s.create_low_level_channel(proxy_to),
        };
        let i_s = self.i_s.clone();
        Ok(async move {
            let url = dest_proxy_url.clone();
            let res = async move {
                let (mut fwd_write, fwd_read) = fut.await?;
                fwd_write
                    .send(ProxyWire::chan_new(url.into()))
                    .await
                    .map_err(TransportError::other)?;
                TransportResult::Ok((fwd_write, fwd_read))
            }
            .await;
            let (fwd_write, fwd_read) = match res {
                Err(e) => {
                    let _ = i_s.prune_bad_proxy_to(dest_proxy_url).await;
                    write
                        .send(ProxyWire::failure(format!("{:?}", e)))
                        .await
                        .map_err(TransportError::other)?;
                    return Ok(());
                }
                Ok(t) => t,
            };
            cross_join_channel_forward(fwd_write, read);
            cross_join_channel_forward(write, fwd_read);
            Ok(())
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
        let fut = self.sub_sender.create_channel(base_url);
        Ok(async move {
            let (_url, write, read) = fut.await?;
            let write = wire_write::wrap_wire_write(write);
            let read = wire_read::wrap_wire_read(read);
            Ok((write, read))
        }
        .boxed()
        .into())
    }

    fn handle_prune_bad_proxy_to(&mut self, proxy_url: ProxyUrl) -> InternalHandlerResult<()> {
        self.proxy_list.remove(&proxy_url);
        Ok(async move { Ok(()) }.boxed().into())
    }

    #[tracing::instrument(skip(self))]
    fn handle_register_proxy_to(
        &mut self,
        proxy_url: ProxyUrl,
        base_url: url2::Url2,
    ) -> InternalHandlerResult<()> {
        // expire ProxyTo entries at double the proxy keepalive timeframe.
        let expires_at = std::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(
                PROXY_KEEPALIVE_MS * KEEPALIVE_MULTIPLIER,
            ))
            .unwrap();
        self.proxy_list.insert(
            proxy_url,
            ProxyTo {
                base_connection_url: base_url,
                expires_at,
            },
        );
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_req_proxy(&mut self, proxy_url: ProxyUrl) -> InternalHandlerResult<()> {
        tracing::info!(
            "{}: wishes to proxy through {}:{}",
            self.this_url.short(),
            proxy_url.short(),
            proxy_url
        );
        let cert_digest = self.tls.cert_digest.clone();
        let fut = self.i_s.create_low_level_channel(proxy_url.into_base());
        let i_s = self.i_s.clone();
        Ok(async move {
            let (mut write, mut read) = fut.await?;

            write
                .send(ProxyWire::req_proxy(cert_digest.to_vec().into()))
                .await
                .map_err(TransportError::other)?;
            let res = match read.next().await {
                None => return Err("no response to proxy request".into()),
                Some(r) => r,
            };
            let proxy_url = match res {
                ProxyWire::ReqProxyOk(p) => p.proxy_url,
                ProxyWire::Failure(f) => {
                    return Err(format!("err response to proxy request: {:?}", f.reason).into());
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
    fn handle_debug(&mut self) -> TransportListenerHandlerResult<serde_json::Value> {
        let url = self.this_url.to_string();
        let sub = self.sub_sender.debug();
        let proxy_count = self.proxy_list.iter().count();
        Ok(async move {
            let sub = sub.await?;
            Ok(serde_json::json! {{
                "sub_transport": sub,
                "url": url,
                "proxy_count": proxy_count,
                "tokio_task_count": kitsune_p2p_types::metrics::metric_task_count(),
                "sys_info": kitsune_p2p_types::metrics::get_sys_info(),
            }})
        }
        .boxed()
        .into())
    }

    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        let this_url: url2::Url2 = (&self.this_url).into();
        Ok(async move { Ok(this_url) }.boxed().into())
    }

    fn handle_create_channel(
        &mut self,
        url: url2::Url2,
    ) -> TransportListenerHandlerResult<(url2::Url2, TransportChannelWrite, TransportChannelRead)>
    {
        let short = self.this_url.short().to_string();
        let proxy_url = ProxyUrl::from(url);
        let tls_client_config = self.tls_client_config.clone();
        let i_s = self.i_s.clone();
        Ok(async move {
            let (mut write, read) = i_s
                .create_low_level_channel(proxy_url.as_base().clone())
                .await?;
            write
                .send(ProxyWire::chan_new(proxy_url.clone().into()))
                .await
                .map_err(TransportError::other)?;
            let ((send1, recv1), (send2, recv2)) = create_transport_channel_pair();
            tls_cli::spawn_tls_client(
                short,
                proxy_url.clone(),
                tls_client_config,
                send1,
                recv1,
                write,
                read,
            )
            .await
            .map_err(TransportError::other)??;
            Ok((proxy_url.into(), send2, recv2))
        }
        .boxed()
        .into())
    }
}
