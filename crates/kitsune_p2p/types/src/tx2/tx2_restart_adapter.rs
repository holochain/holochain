//! Wrap a tx2 backend adapter in endpoint close / binding restart backoff code.

use crate::tx2::tx2_adapter::*;
use crate::tx2::tx2_utils::*;
use crate::*;

use futures::future::{BoxFuture, FutureExt};
use futures::stream::{BoxStream, StreamExt};
use parking_lot::RwLock;

/// Wrap a tx2 backend adapter in endpoint close / binding restart backoff code.
/// If you call "close" on this backend, it *will* close, but will immediately
/// start trying to re-bind. The ConRecvAdapt stream will never end, it will
/// transition to messages from the new binding.
pub fn tx2_restart_adapter(sub_adapter: AdapterFactory) -> AdapterFactory {
    Arc::new(RestartBackendAdapt(sub_adapter))
}

// -- private -- //

struct CrState {
    sub_factory: AdapterFactory,
    bind_url: TxUrl,
    sub_ep: Arc<RwLock<Option<Arc<dyn EndpointAdapt>>>>,
    con_recv: Option<Box<dyn ConRecvAdapt>>,
}

impl CrState {
    async fn check_bind(&mut self) {
        if self.con_recv.is_some() {
            return;
        }
        match self
            .sub_factory
            .bind(self.bind_url.clone(), KitsuneTimeout::from_millis(10000))
            .await
        {
            Ok((ep, con_recv)) => {
                tracing::info!("tx2_restart_adapter bound {:?}", ep.local_addr());
                self.con_recv = Some(con_recv);
                self.sub_ep.write().replace(ep);
            }
            Err(e) => {
                tracing::warn!("bind error, will retry: {:?}", e);
            }
        }
    }
}

struct RestartConRecvAdapt(BoxStream<'static, ConFut>);

impl RestartConRecvAdapt {
    pub async fn new(
        sub_factory: AdapterFactory,
        bind_url: TxUrl,
        sub_ep: Arc<RwLock<Option<Arc<dyn EndpointAdapt>>>>,
    ) -> Self {
        let mut state = CrState {
            sub_factory,
            bind_url,
            sub_ep,
            con_recv: None,
        };

        state.check_bind().await;

        Self(
            futures::stream::unfold(state, move |mut state| async move {
                let item = loop {
                    if state.con_recv.is_none() {
                        let mut backoff_ms = 10;

                        loop {
                            state.check_bind().await;

                            if state.con_recv.is_some() {
                                break;
                            }

                            backoff_ms *= 2;
                            if backoff_ms >= 5000 {
                                backoff_ms = 5000;
                            }

                            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        }
                    }

                    match state.con_recv.as_mut().unwrap().next().await {
                        Some(item) => break item,
                        None => {
                            state.con_recv = None;
                            continue;
                        }
                    }
                };

                Some((item, state))
            })
            .boxed(),
        )
    }
}

impl futures::stream::Stream for RestartConRecvAdapt {
    type Item = ConFut;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

impl ConRecvAdapt for RestartConRecvAdapt {}

struct RestartEndpointAdapt {
    sub_ep: Arc<RwLock<Option<Arc<dyn EndpointAdapt>>>>,
    uniq: Uniq,
    local_cert: Tx2Cert,
}

impl EndpointAdapt for RestartEndpointAdapt {
    fn debug(&self) -> serde_json::Value {
        // TODO - more useful info about the retry timing?
        if let Some(ep) = &*self.sub_ep.read() {
            serde_json::json!({
                "type": "tx2_restart",
                "state": "open",
                "sub_ep": ep.debug(),
            })
        } else {
            serde_json::json!({
                "type": "tx2_restart",
                "state": "closed",
            })
        }
    }

    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        if let Some(ep) = &*self.sub_ep.read() {
            ep.local_addr()
        } else {
            Err("currently closed".into())
        }
    }

    fn local_cert(&self) -> Tx2Cert {
        self.local_cert.clone()
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        if let Some(ep) = &*self.sub_ep.read() {
            ep.connect(url, timeout)
        } else {
            async move { Err("currently closed".into()) }.boxed()
        }
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        if let Some(ep) = &*self.sub_ep.read() {
            ep.close(code, reason)
        } else {
            async move {}.boxed()
        }
    }
}

struct RestartBackendAdapt(AdapterFactory);

impl BindAdapt for RestartBackendAdapt {
    fn bind(&self, url: TxUrl, _timeout: KitsuneTimeout) -> EndpointFut {
        let sub_fact = self.0.clone();
        async move {
            let local_cert = sub_fact.local_cert();
            let sub_ep = Arc::new(RwLock::new(None));
            let con_recv = RestartConRecvAdapt::new(sub_fact, url, sub_ep.clone()).await;
            let ep: Arc<dyn EndpointAdapt> = Arc::new(RestartEndpointAdapt {
                sub_ep,
                uniq: Uniq::default(),
                local_cert,
            });
            let con_recv: Box<dyn ConRecvAdapt> = Box::new(con_recv);
            Ok((ep, con_recv))
        }
        .boxed()
    }

    fn local_cert(&self) -> Tx2Cert {
        self.0.local_cert()
    }
}
