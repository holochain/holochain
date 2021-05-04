use crate::*;

use futures::future::{BoxFuture, FutureExt};
use ghost_actor::GhostControlSender;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::*;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::tx2::tx2_utils::*;
use types::direct::*;
use types::kdentry::KdEntry;
use types::kdhash::KdHash;
use types::persist::*;

/// create a new v1 instance of the kitsune direct api
pub async fn new_kitsune_direct_v1(
    // persistence module to use for this kdirect instance
    persist: KdPersist,

    // v1 is only set up to run through a proxy
    // specify the proxy addr here
    proxy: TxUrl,
) -> KitsuneResult<(
    KitsuneDirect,
    Box<dyn futures::Stream<Item = KitsuneDirectEvt>>,
)> {
    let mut config = KitsuneP2pConfig::default();

    let tuning_params = config.tuning_params.clone();

    config.transport_pool.push(TransportConfig::Proxy {
        sub_transport: Box::new(TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }),
        proxy_config: ProxyConfig::RemoteProxyClient {
            proxy_url: proxy.into(),
        },
    });

    let tls = persist.singleton_tls_config().await?;

    let (p2p, _evt) = spawn_kitsune_p2p(config, tls)
        .await
        .map_err(KitsuneError::other)?;

    let logic_chan = LogicChan::new(tuning_params.concurrent_limit_per_thread);

    let kdirect = KitsuneDirect(Kd1::new(persist, p2p));

    Ok((kdirect, Box::new(logic_chan)))
}

// -- private -- //

struct Kd1Inner {
    p2p: ghost_actor::GhostSender<actor::KitsuneP2p>,
}

#[derive(Clone)]
struct Kd1 {
    uniq: Uniq,
    persist: KdPersist,
    inner: Share<Kd1Inner>,
}

impl Kd1 {
    pub fn new(persist: KdPersist, p2p: ghost_actor::GhostSender<actor::KitsuneP2p>) -> Arc<Self> {
        Arc::new(Self {
            uniq: Uniq::default(),
            persist,
            inner: Share::new(Kd1Inner { p2p }),
        })
    }
}

impl AsKitsuneDirect for Kd1 {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        // TODO - pass along code/reason to transport shutdowns
        let r = self.inner.share_mut(|i, c| {
            *c = true;
            Ok(i.p2p.clone())
        });
        async move {
            if let Ok(p2p) = r {
                let _ = p2p.ghost_actor_shutdown_immediate().await;
            }
        }
        .boxed()
    }

    fn get_persist(&self) -> KdPersist {
        self.persist.clone()
    }

    fn list_transport_bindings(&self) -> BoxFuture<'static, KitsuneResult<Vec<TxUrl>>> {
        let fut = self
            .inner
            .share_mut(|i, _| Ok(i.p2p.list_transport_bindings()));
        async move {
            let res = fut?.await.map_err(KitsuneError::other)?;
            Ok(res.into_iter().map(|u| u.into()).collect())
        }
        .boxed()
    }

    fn join(&self, root: KdHash, agent: KdHash) -> BoxFuture<'static, KitsuneResult<()>> {
        let fut = self
            .inner
            .share_mut(|i, _| Ok(i.p2p.join(root.into(), agent.into())));
        async move {
            fut?.await.map_err(KitsuneError::other)?;
            Ok(())
        }
        .boxed()
    }

    fn message(
        &self,
        root: KdHash,
        from_agent: KdHash,
        to_agent: KdHash,
        content: serde_json::Value,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.inner.clone();
        async move {
            let payload = serde_json::json!(["message", content]);
            let payload = serde_json::to_string(&payload).map_err(KitsuneError::other)?;
            let payload = payload.into_bytes();
            let res = inner
                .share_mut(|i, _| {
                    Ok(i.p2p.rpc_single(
                        root.into(),
                        to_agent.into(),
                        from_agent.into(),
                        payload,
                        None,
                    ))
                })?
                .await
                .map_err(KitsuneError::other)?;
            if res != b"success" {
                return Err(format!("bad response: {}", String::from_utf8_lossy(&res)).into());
            }
            Ok(())
        }
        .boxed()
    }

    fn publish_entry(&self, root: KdHash, entry: KdEntry) -> BoxFuture<'static, KitsuneResult<()>> {
        // TODO - someday this should actually publish...
        //        for now, we are just relying on gossip
        self.persist.store_entry(root, entry).boxed()
    }
}
