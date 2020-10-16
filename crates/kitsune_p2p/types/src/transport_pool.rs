//! Unify multiple sub-transports into one pool.

use crate::transport::*;
use futures::{future::FutureExt, sink::SinkExt, stream::StreamExt};
use ghost_actor::{
    dependencies::{must_future::MustBoxFuture, tracing},
    GhostControlSender,
};
use std::collections::HashMap;

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

    builder
        .spawn(Inner {
            i_s,
            sub_listeners: HashMap::new(),
            evt_send,
        })
        .await?;

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
            tracing::warn!("proxy listener actor SHUTDOWN");
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
        self.sub_listeners.insert(scheme, sub_listener);
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

            tokio::task::spawn(async move {
                while let Some(evt) = sub_event.next().await {
                    if evt_send.send(evt).await.is_err() {
                        break;
                    }
                }
            });

            Ok(())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<TransportListener> for Inner {}

impl TransportListenerHandler for Inner {
    fn handle_debug(&mut self) -> TransportListenerHandlerResult<serde_json::Value> {
        unimplemented!()
    }

    fn handle_bound_url(&mut self) -> TransportListenerHandlerResult<url2::Url2> {
        unimplemented!()
    }

    fn handle_create_channel(
        &mut self,
        _url: url2::Url2,
    ) -> TransportListenerHandlerResult<(url2::Url2, TransportChannelWrite, TransportChannelRead)>
    {
        unimplemented!()
    }
}
