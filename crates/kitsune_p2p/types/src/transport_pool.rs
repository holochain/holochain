//! Unify multiple sub-transports into one pool.

use crate::transport::*;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use ghost_actor::dependencies::tracing;
use ghost_actor::GhostControlSender;
use std::collections::HashMap;

const MAX_TRANSPORTS: usize = 1000;

ghost_actor::ghost_chan! {
    /// Additional control functions for a transport pool
    pub chan TransportPool<TransportError> {
        /// Push a new sub-transport listener into the pool
        fn push_sub_transport(
            sub_listener: ghost_actor::GhostSender<TransportListener>,
            sub_event: TransportEventReceiver,
        ) -> ();
    }
}

/// Spawn a new transport pool actor.
pub async fn spawn_transport_pool() -> TransportResult<(
    ghost_actor::GhostSender<TransportPool>,
    ghost_actor::GhostSender<TransportListener>,
    TransportEventReceiver,
)> {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let i_s = channel_factory.create_channel::<InnerChan>().await?;
    let pool = channel_factory.create_channel::<TransportPool>().await?;
    let listener = channel_factory
        .create_channel::<TransportListener>()
        .await?;

    let (evt_send, evt_recv) = futures::channel::mpsc::channel(10);

    crate::metrics::metric_task(
        spawn_pressure::spawn_limit!(MAX_TRANSPORTS),
        builder.spawn(Inner {
            i_s,
            sub_listeners: HashMap::new(),
            evt_send,
        }),
    )
    .await;

    Ok((pool, listener, evt_recv))
}

ghost_actor::ghost_chan! {
    chan InnerChan<TransportError> {
        fn inject_listener(
            scheme: String,
            sub_listener: ghost_actor::GhostSender<TransportListener>,
        ) -> ();
    }
}

struct Inner {
    i_s: ghost_actor::GhostSender<InnerChan>,
    sub_listeners: HashMap<String, ghost_actor::GhostSender<TransportListener>>,
    evt_send: TransportEventSender,
}

impl ghost_actor::GhostControlHandler for Inner {
    fn handle_ghost_actor_shutdown(mut self) -> MustBoxFuture<'static, ()> {
        async move {
            for (_, sub) in self.sub_listeners {
                let _ = sub.ghost_actor_shutdown().await;
            }
            self.evt_send.close_channel();
            tracing::warn!("transport pool actor SHUTDOWN");
        }
        .boxed()
        .into()
    }
}

impl ghost_actor::GhostHandler<InnerChan> for Inner {}

impl InnerChanHandler for Inner {
    fn handle_inject_listener(
        &mut self,
        scheme: String,
        sub_listener: ghost_actor::GhostSender<TransportListener>,
    ) -> InnerChanHandlerResult<()> {
        match self.sub_listeners.entry(scheme.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return Err(format!("scheme '{}' already mapped in this pool", scheme,).into());
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(sub_listener);
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<TransportPool> for Inner {}

impl TransportPoolHandler for Inner {
    fn handle_push_sub_transport(
        &mut self,
        sub_listener: ghost_actor::GhostSender<TransportListener>,
        mut sub_event: TransportEventReceiver,
    ) -> TransportPoolHandlerResult<()> {
        let i_s = self.i_s.clone();
        let mut evt_send = self.evt_send.clone();

        Ok(async move {
            let scheme = sub_listener.bound_url().await?;
            let scheme = scheme.scheme().to_string();

            i_s.inject_listener(scheme, sub_listener).await?;

            crate::metrics::metric_task(spawn_pressure::spawn_limit!(500), async move {
                while let Some(evt) = sub_event.next().await {
                    if evt_send.send(evt).await.is_err() {
                        break;
                    }
                }

                <Result<(), ()>>::Ok(())
            })
            .await;

            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<TransportListener> for Inner {}

impl TransportListenerHandler for Inner {
    fn handle_debug(&mut self) -> TransportListenerHandlerResult<serde_json::Value> {
        let out = self
            .sub_listeners
            .iter()
            .map(|(k, v)| {
                let k = k.to_string();
                let v = v.debug();
                async move { TransportResult::Ok((k, v.await?)) }
            })
            .collect::<Vec<_>>();
        Ok(async move {
            let v = futures::future::try_join_all(out).await?;
            let m = v
                .into_iter()
                .collect::<serde_json::map::Map<String, serde_json::Value>>();
            Ok(m.into())
        }
        .boxed()
        .into())
    }

    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        let urls = self
            .sub_listeners
            .iter()
            .map(|(k, v)| {
                let k = k.to_string();
                let v = v.bound_url();
                async move { TransportResult::Ok((k, v.await?)) }
            })
            .collect::<Vec<_>>();
        Ok(async move {
            let urls = futures::future::try_join_all(urls).await?;
            let mut out = url2::url2!("kitsune-pool:pool");
            {
                let mut query = out.query_pairs_mut();
                for (k, v) in urls {
                    query.append_pair(&k, v.as_str());
                }
            }
            Ok(out)
        }
        .boxed()
        .into())
    }

    fn handle_create_channel(
        &mut self,
        url: url2::Url2,
    ) -> TransportListenerHandlerResult<(url2::Url2, TransportChannelWrite, TransportChannelRead)>
    {
        // TODO - right now requiring sub transport scheme to create channel
        //        would be nice to also accept a pool url && prioritize the
        //        sub-scheme.
        let scheme = url.scheme().to_string();
        match self.sub_listeners.get(&scheme) {
            None => Err(format!("no sub-transport matching scheme '{}' in pool", scheme).into()),
            Some(s) => Ok(s.create_channel(url)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::throughput;
    use crate::transport_mem::*;
    use futures::stream::StreamExt;

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

    #[tokio::test(threaded_scheduler)]
    async fn it_can_pool_transport() -> TransportResult<()> {
        let _ = ghost_actor::dependencies::tracing::subscriber::set_global_default(
            tracing_subscriber::FmtSubscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .finish(),
        );

        // create stacked Pool<Mem> #1
        let (c1, p1, e1) = spawn_transport_pool().await?;
        let (sub1, sube1) = spawn_bind_transport_mem().await?;
        let suburl1 = sub1.bound_url().await?;
        tracing::warn!(?suburl1);
        c1.push_sub_transport(sub1, sube1).await?;
        test_receiver(e1);

        // create stacked Pool<Mem> #2
        let (c2, p2, e2) = spawn_transport_pool().await?;
        let (sub2, sube2) = spawn_bind_transport_mem().await?;
        let suburl2 = sub2.bound_url().await?;
        tracing::warn!(?suburl2);
        c2.push_sub_transport(sub2, sube2).await?;
        test_receiver(e2);

        let url1 = p1.bound_url().await?;
        tracing::warn!(?url1);
        let url2 = p2.bound_url().await?;
        tracing::warn!(?url2);

        // send a request to #2 through #1 - get the response
        let res = p1.request(suburl2.clone(), b"test1".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test1", suburl1),
            &String::from_utf8_lossy(&res),
        );

        // send a request to #1 through #2 - get the response
        let res = p2.request(suburl1.clone(), b"test2".to_vec()).await?;
        assert_eq!(
            &format!("echo({}): test2", suburl2),
            &String::from_utf8_lossy(&res),
        );

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn pool_tp() -> TransportResult<()> {
        let (c1, p1, e1) = spawn_transport_pool().await?;
        let (sub1, sube1) = spawn_bind_transport_mem().await?;
        let suburl1 = sub1.bound_url().await?;
        c1.push_sub_transport(sub1, sube1).await?;
        test_receiver(e1);

        let _ = p1.bound_url().await?;

        let bytes = 100;
        let p1 = &p1;
        throughput(bytes, 10, || {
            let suburl1 = suburl1.clone();
            async move {
                let _ = p1
                    .request(suburl1, vec![0u8; bytes as usize])
                    .await
                    .unwrap();
            }
        })
        .await;

        Ok(())
    }
}
