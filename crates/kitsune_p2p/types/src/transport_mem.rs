//! A mem-only transport - largely for testing

use crate::transport::{transport_connection::*, transport_listener::*, *};
use futures::future::FutureExt;

use once_cell::sync::Lazy;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::Mutex;

const SCHEME: &str = "kitsune-mem";

type CoreSender = futures::channel::mpsc::Sender<TransportListenerEvent>;

static CORE: Lazy<Arc<Mutex<HashMap<url2::Url2, CoreSender>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

async fn get_core(url: url2::Url2) -> TransportResult<CoreSender> {
    let lock = CORE.lock().await;
    lock.get(&url)
        .ok_or_else(|| format!("bad core: {}", url).into())
        .map(|v| v.clone())
}

async fn put_core(url: url2::Url2, send: CoreSender) -> TransportResult<()> {
    let mut lock = CORE.lock().await;
    match lock.entry(url.clone()) {
        Entry::Vacant(e) => {
            e.insert(send);
            Ok(())
        }
        Entry::Occupied(_) => Err(format!("core {} already exists", url).into()),
    }
}

fn drop_core(url: url2::Url2) {
    tokio::task::spawn(async move {
        let mut lock = CORE.lock().await;
        lock.remove(&url);
    });
}

/// Spawn / bind the listening side of a mem-only transport - largely for testing
pub async fn spawn_bind_transport_mem() -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportListenerEventReceiver,
)> {
    let url = url2::url2!("{}://{}", SCHEME, nanoid::nanoid!(),);

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TransportListener>()
        .await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    put_core(url.clone(), evt_send).await?;

    tokio::task::spawn(builder.spawn(InnerListen::new(url)));

    Ok((sender, evt_recv))
}

struct InnerListen {
    url: url2::Url2,
}

impl Drop for InnerListen {
    fn drop(&mut self) {
        drop_core(self.url.clone());
    }
}

impl InnerListen {
    pub fn new(url: url2::Url2) -> Self {
        Self { url }
    }
}

impl ghost_actor::GhostControlHandler for InnerListen {}

impl ghost_actor::GhostHandler<TransportListener> for InnerListen {}

impl TransportListenerHandler for InnerListen {
    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        let url = self.url.clone();
        Ok(async move { Ok(url) }.boxed().into())
    }

    fn handle_connect(
        &mut self,
        url: url2::Url2,
    ) -> TransportListenerHandlerResult<(
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        let this_url = self.url.clone();
        Ok(async move {
            let evt_send = get_core(url.clone()).await?;

            let (send1, evt1, send2, evt2) = InnerCon::spawn(this_url, url).await?;

            evt_send.incoming_connection(send1, evt1).await?;

            Ok((send2, evt2))
        }
        .boxed()
        .into())
    }
}

struct InnerCon {
    evt_send: futures::channel::mpsc::Sender<TransportConnectionEvent>,
    this_url: url2::Url2,
    remote_url: url2::Url2,
}

impl InnerCon {
    pub async fn spawn(
        url_a: url2::Url2,
        url_b: url2::Url2,
    ) -> TransportResult<(
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
        ghost_actor::GhostSender<TransportConnection>,
        TransportConnectionEventReceiver,
    )> {
        let (evt_send1, evt_recv1) = futures::channel::mpsc::channel(10);
        let (evt_send2, evt_recv2) = futures::channel::mpsc::channel(10);

        let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

        let sender1 = builder
            .channel_factory()
            .create_channel::<TransportConnection>()
            .await?;

        tokio::task::spawn(builder.spawn(InnerCon::new(evt_send2, url_b.clone(), url_a.clone())));

        let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

        let sender2 = builder
            .channel_factory()
            .create_channel::<TransportConnection>()
            .await?;

        tokio::task::spawn(builder.spawn(InnerCon::new(evt_send1, url_a, url_b)));

        Ok((sender1, evt_recv1, sender2, evt_recv2))
    }

    pub fn new(
        evt_send: futures::channel::mpsc::Sender<TransportConnectionEvent>,
        this_url: url2::Url2,
        remote_url: url2::Url2,
    ) -> Self {
        Self {
            evt_send,
            this_url,
            remote_url,
        }
    }
}

impl ghost_actor::GhostControlHandler for InnerCon {}

impl ghost_actor::GhostHandler<TransportConnection> for InnerCon {}

impl TransportConnectionHandler for InnerCon {
    fn handle_remote_url(&mut self) -> TransportConnectionHandlerResult<url2::Url2> {
        let url = self.remote_url.clone();
        Ok(async move { Ok(url) }.boxed().into())
    }

    fn handle_create_channel(&mut self) -> TransportConnectionHandlerResult<(
        TransportChannelWrite,
        TransportChannelRead,
    )> {
        let this_url = self.this_url.clone();
        let evt_send = self.evt_send.clone();
        Ok(async move {
            let (recv1, send1) = tokio::io::split(std::io::Cursor::new(Vec::new()));
            let (recv2, send2) = tokio::io::split(std::io::Cursor::new(Vec::new()));
            evt_send.incoming_channel(this_url, Box::new(send1), Box::new(recv2)).await?;
            let send2: TransportChannelWrite = Box::new(send2);
            let recv1: TransportChannelRead = Box::new(recv1);
            Ok((send2, recv1))
        }.boxed().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::StreamExt;

    fn handle_connection_event(mut recv: TransportConnectionEventReceiver) {
        tokio::task::spawn(async move {
            while let Some(msg) = recv.next().await {
                match msg {
                    TransportConnectionEvent::IncomingChannel {
                        respond, url, mut send, mut recv, ..
                    } => {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};
                        respond.respond(Ok(async move {
                            let mut data = Vec::new();
                            recv.read_to_end(&mut data).await.map_err(TransportError::other)?;
                            let data = format!("echo({}): {}", url, String::from_utf8_lossy(&data),)
                                .into_bytes();
                            send.write_all(&data).await.map_err(TransportError::other)?;
                            send.shutdown().await.map_err(TransportError::other)?;
                            Ok(())
                        }.boxed().into()));
                    }
                }
            }
        });
    }

    fn handle_listener_event(mut recv: TransportListenerEventReceiver) {
        tokio::task::spawn(async move {
            while let Some(msg) = recv.next().await {
                match msg {
                    TransportListenerEvent::IncomingConnection {
                        respond, receiver, ..
                    } => {
                        handle_connection_event(receiver);
                        respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                    }
                }
            }
        });
    }

    #[tokio::test(threaded_scheduler)]
    async fn it_can_mem_transport() -> TransportResult<()> {
        let (bind1, evt1) = spawn_bind_transport_mem().await?;
        handle_listener_event(evt1);
        let (bind2, evt2) = spawn_bind_transport_mem().await?;
        handle_listener_event(evt2);

        let url1 = bind1.bound_url().await?;
        let url2 = bind2.bound_url().await?;

        let (con1, con_evt1) = bind1.connect(url2.clone()).await?;
        handle_connection_event(con_evt1);
        let (con2, con_evt2) = bind2.connect(url1.clone()).await?;
        handle_connection_event(con_evt2);

        assert_eq!(url2, con1.remote_url().await?);
        assert_eq!(url1, con2.remote_url().await?);

        let res = con1.request(b"test1".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test1", url1),
            &String::from_utf8_lossy(&res),
        );

        let res = con2.request(b"test2".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test2", url2),
            &String::from_utf8_lossy(&res),
        );

        Ok(())
    }
}
