//! A mem-only transport - largely for testing

use crate::transport::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;

use once_cell::sync::Lazy;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const SCHEME: &str = "kitsune-mem";
const MAX_TRANSPORTS: usize = 1000;
const MAX_CHANNELS: usize = 500;

static CORE: Lazy<Arc<Mutex<HashMap<url2::Url2, TransportEventSender>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

async fn list_cores() -> TransportResult<Vec<url2::Url2>> {
    let lock = CORE.lock().await;
    Ok(lock.keys().cloned().collect())
}

async fn get_core(url: url2::Url2) -> TransportResult<TransportEventSender> {
    let lock = CORE.lock().await;
    lock.get(&url)
        .ok_or_else(|| format!("bad core: {}", url).into())
        .map(|v| v.clone())
}

async fn put_core(url: url2::Url2, send: TransportEventSender) -> TransportResult<()> {
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
    crate::metrics::metric_task_warn_limit(spawn_pressure::spawn_limit!(500), async move {
        let mut lock = CORE.lock().await;
        lock.remove(&url);

        <Result<(), ()>>::Ok(())
    });
}

/// Spawn / bind the listening side of a mem-only transport - largely for testing
pub async fn spawn_bind_transport_mem() -> TransportResult<(
    ghost_actor::GhostSender<TransportListener>,
    TransportEventReceiver,
)> {
    let url = url2::url2!("{}://{}", SCHEME, nanoid::nanoid!());

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<TransportListener>()
        .await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    put_core(url.clone(), evt_send).await?;

    crate::metrics::metric_task(
        spawn_pressure::spawn_limit!(MAX_TRANSPORTS),
        builder.spawn(InnerListen::new(url)),
    )
    .await;

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
    fn handle_debug(&mut self) -> TransportListenerHandlerResult<serde_json::Value> {
        let url = self.url.clone();
        let listeners = list_cores();
        Ok(async move {
            let listeners = listeners.await?;
            Ok(serde_json::json! {{
                "url": url,
                "listener_count": listeners.len(),
            }})
        }
        .boxed()
        .into())
    }

    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        let url = self.url.clone();
        Ok(async move { Ok(url) }.boxed().into())
    }

    fn handle_create_channel(
        &mut self,
        url: url2::Url2,
    ) -> TransportListenerHandlerResult<(url2::Url2, TransportChannelWrite, TransportChannelRead)>
    {
        let this_url = self.url.clone();
        Ok(async move {
            let mut evt_send = get_core(url.clone()).await?;

            let ((send1, recv1), (send2, recv2)) = create_transport_channel_pair();

            // if we don't spawn here there can be a deadlock on
            // incoming_channel trying to process all channel data
            // before we've returned our halves here.
            crate::metrics::metric_task(spawn_pressure::spawn_limit!(MAX_CHANNELS), async move {
                // it's ok if this errors... the channels will close.
                let _ = evt_send
                    .send(TransportEvent::IncomingChannel(this_url, send1, recv1))
                    .await;

                <Result<(), ()>>::Ok(())
            })
            .await;
            Ok((url, send2, recv2))
        }
        .boxed()
        .into())
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::throughput;

    use super::*;
    use futures::stream::StreamExt;
    pub type TransportEventReceiverFast2 = tokio::sync::mpsc::Receiver<TransportEventFast>;

    fn test_receiver(mut recv: TransportEventReceiver) {
        crate::metrics::metric_task_warn_limit(spawn_pressure::spawn_limit!(500), async move {
            while let Some(evt) = recv.next().await {
                match evt {
                    TransportEvent::IncomingChannel(url, mut write, read) => {
                        let data = read.read_to_end().await;
                        let data = format!("echo({}): {}", url, String::from_utf8_lossy(&data),);
                        write.write_and_close(data.into_bytes()).await?;
                    }
                }
            }
            TransportResult::Ok(())
        });
    }

    fn test_receiver_fast(mut recv: TransportEventReceiverFast2) {
        crate::metrics::metric_task_warn_limit(spawn_pressure::spawn_limit!(500), async move {
            while let Some(evt) = recv.next().await {
                match evt {
                    TransportEventFast::IncomingChannel(_url, write, read) => {
                        let data = read.await.unwrap();
                        // let data = format!("echo({}): {}", url, String::from_utf8_lossy(&data),);
                        write.send(data).unwrap();
                    }
                }
            }
            TransportResult::Ok(())
        });
    }

    fn test_receiver_none(mut recv: TransportEventReceiverFast2) {
        crate::metrics::metric_task_warn_limit(spawn_pressure::spawn_limit!(500), async move {
            while let Some(_) = recv.next().await {}
            TransportResult::Ok(())
        });
    }

    #[tokio::test(threaded_scheduler)]
    async fn it_can_mem_transport() -> TransportResult<()> {
        let (bind1, evt1) = spawn_bind_transport_mem().await?;
        test_receiver(evt1);
        let (bind2, evt2) = spawn_bind_transport_mem().await?;
        test_receiver(evt2);

        let url1 = bind1.bound_url().await?;
        let url2 = bind2.bound_url().await?;

        let res = bind1.request(url2.clone(), b"test1".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test1", url1),
            &String::from_utf8_lossy(&res),
        );

        let res = bind2.request(url1.clone(), b"test2".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test2", url2),
            &String::from_utf8_lossy(&res),
        );

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn mem_tp() -> TransportResult<()> {
        let (bind1, evt1) = spawn_bind_transport_mem().await?;
        test_receiver(evt1);

        let url1 = bind1.bound_url().await?;

        let bytes = 100;
        let bind1 = &bind1;
        throughput(bytes, 1, || {
            let url1 = url1.clone();
            async move {
                let _ = bind1
                    .request(url1, vec![0u8; bytes as usize])
                    .await
                    .unwrap();
            }
        })
        .await;

        let (to_test_recv, evt2) = futures::channel::mpsc::channel(10);
        test_receiver(evt2);

        throughput(bytes, 1, move || {
            let (tx_write, rx_write) = futures::channel::mpsc::channel(10);
            let (tx_read, rx_read) = futures::channel::mpsc::channel(10);
            let evt = TransportEvent::IncomingChannel(
                url1.clone(),
                Box::new(tx_write.sink_map_err(|e| TransportError::Other(e.into()))),
                Box::new(rx_read),
            );
            let mut tx_read = Box::new(tx_read.sink_map_err(|e| TransportError::Other(e.into())));
            let mut to_test_recv = to_test_recv.clone();
            async move {
                to_test_recv.send(evt).await.unwrap();
                let data = vec![0u8; bytes as usize];
                tx_read.write_and_close(data).await.unwrap();
                rx_write.read_to_end().await;
            }
        })
        .await;

        let (to_test_recv, evt2) = tokio::sync::mpsc::channel(10);
        test_receiver_fast(evt2);

        let url1 = bind1.bound_url().await?;

        throughput(bytes, 1, move || {
            let (tx_write, rx_write) = tokio::sync::oneshot::channel();
            let (tx_read, rx_read) = tokio::sync::oneshot::channel();
            let evt = TransportEventFast::IncomingChannel(url1.clone(), tx_write, rx_read);
            // let mut tx_read = Box::new(tx_read.sink_map_err(|e| TransportError::Other(e.into())));
            let mut to_test_recv = to_test_recv.clone();
            async move {
                to_test_recv.send(evt).await.ok();
                let data = vec![0u8; bytes as usize];
                tx_read.send(data).unwrap();
                let r = rx_write.await.unwrap();
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        let (to_test_recv, evt2) = tokio::sync::mpsc::channel(10);
        test_receiver_none(evt2);
        let url1 = bind1.bound_url().await?;

        throughput(bytes, 1, move || {
            let (tx_write, _rx_write) = tokio::sync::oneshot::channel();
            let (_tx_read, rx_read) = tokio::sync::oneshot::channel();
            let evt = TransportEventFast::IncomingChannel(url1.clone(), tx_write, rx_read);
            // let mut tx_read = Box::new(tx_read.sink_map_err(|e| TransportError::Other(e.into())));
            let mut to_test_recv = to_test_recv.clone();
            async move {
                to_test_recv.send(evt).await.ok();
                let _data = vec![0u8; bytes as usize];
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, rx) = futures::channel::mpsc::channel(10);
            let mut tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            async move {
                let data = vec![0u8; bytes as usize];
                tx.write_and_close(data).await.unwrap();
                let r = rx.read_to_end().await;
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, rx) = tokio::sync::oneshot::channel();
            async move {
                let data = vec![0u8; bytes as usize];
                tx.send(data).unwrap();
                let r = rx.await.unwrap();
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, mut rx) = futures::channel::mpsc::channel(10);
            let mut tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            async move {
                let data = vec![0u8; bytes as usize];
                tx.send(data).await.unwrap();
                tx.close().await.unwrap();

                let r: Vec<_> = rx.next().await.unwrap();
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, rx) = futures::channel::mpsc::channel(10);
            let mut tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            async move {
                let data = vec![0u8; bytes as usize];
                tx.write_and_close(data).await.unwrap();

                let r: Vec<u8> = rx.flat_map(|f| futures::stream::iter(f)).collect().await;
                // let r: Vec<_> = r.into_iter().flatten().collect();
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, rx) = futures::channel::mpsc::channel(bytes as usize + 10);
            let tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            async move {
                let data = vec![0u8; bytes as usize];
                let data = futures::stream::iter(data.into_iter().map(|d| Ok(d)));
                data.forward(tx).await.unwrap();

                let r: Vec<u8> = rx.collect().await;
                // let r: Vec<_> = r.into_iter().flatten().collect();
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        throughput(bytes, 1, move || {
            let (tx, rx) = futures::channel::mpsc::channel(10);
            let mut tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            async move {
                // let buf = &mut buf;
                let buf = Vec::with_capacity(bytes as usize);
                let data = vec![0u8; bytes as usize];
                tx.write_and_close(data).await.unwrap();

                let r: Vec<u8> = rx
                    .fold(buf, |mut acc, x| async move {
                        acc.extend(x);
                        acc
                    })
                    .await;
                assert_eq!(r.len(), bytes as usize);
            }
        })
        .await;

        let buf = Vec::with_capacity(bytes as usize);
        let buf = Arc::new(tokio::sync::Mutex::new(buf));
        throughput(bytes, 1, || {
            let (tx, mut rx) = futures::channel::mpsc::channel(10);
            let mut tx = Box::new(tx.sink_map_err(|e| TransportError::Other(e.into())));
            let buf = buf.clone();
            async move {
                // let buf = &mut buf;
                let data = vec![0u8; bytes as usize];
                tx.write_and_close(data).await.unwrap();

                // let r: &mut Vec<u8> = rx
                //     .fold(buf, |mut acc, x| async move {
                //         acc.extend(x);
                //         acc
                //     })
                //     .await;
                let mut buf = buf.lock().await;
                buf.extend(rx.next().await.unwrap());
                assert_eq!(buf.len(), bytes as usize);
                buf.clear();
            }
        })
        .await;
        Ok(())
    }
}
