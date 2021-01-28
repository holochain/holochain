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
    crate::metric_task!(async move {
        let mut lock = CORE.lock().await;
        lock.remove(&url);
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

    crate::metric_task!(builder.spawn(InnerListen::new(url)));

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
                "listeners": listeners,
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
            crate::metric_task!(async move {
                // it's ok if this errors... the channels will close.
                let _ = evt_send
                    .send(TransportEvent::IncomingChannel(this_url, send1, recv1))
                    .await;
            });
            Ok((url, send2, recv2))
        }
        .boxed()
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::StreamExt;

    fn test_receiver(mut recv: TransportEventReceiver) {
        crate::metric_task!(async move {
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
}
