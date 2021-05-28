use crate::*;

use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use ghost_actor::GhostControlSender;
//use ghost_actor::dependencies::tracing;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::event::*;
use kitsune_p2p::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::tx2::tx2_utils::*;
use types::direct::*;
use types::kdagent::KdAgentInfo;
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
    Box<dyn futures::Stream<Item = KitsuneDirectEvt> + 'static + Send + Unpin>,
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

    let (p2p, evt) = spawn_kitsune_p2p(config, tls)
        .await
        .map_err(KitsuneError::other)?;

    let logic_chan = LogicChan::new(tuning_params.concurrent_limit_per_thread);
    let lhnd = logic_chan.handle().clone();

    let kdirect = Kd1::new(persist, p2p);

    logic_chan
        .handle()
        .clone()
        .capture_logic(handle_events(tuning_params, kdirect.clone(), lhnd, evt))
        .await?;

    let kdirect = KitsuneDirect(kdirect);

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

    fn publish_entry(
        &self,
        root: KdHash,
        agent: KdHash,
        entry: KdEntry,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        // TODO - someday this should actually publish...
        //        for now, we are just relying on gossip
        self.persist.store_entry(root, agent, entry).boxed()
    }
}

async fn handle_events(
    tuning_params: KitsuneP2pTuningParams,
    kdirect: Arc<Kd1>,
    lhnd: LogicChanHandle<KitsuneDirectEvt>,
    evt: futures::channel::mpsc::Receiver<event::KitsuneP2pEvent>,
) {
    use futures::future::TryFutureExt;
    let kdirect = &kdirect;
    let lhnd = &lhnd;

    evt.for_each_concurrent(
        tuning_params.concurrent_limit_per_thread,
        move |evt| async move {
            match evt {
                event::KitsuneP2pEvent::PutAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_put_agent_info_signed(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::GetAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_get_agent_info_signed(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::QueryAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_query_agent_info_signed(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::PutMetricDatum {
                    respond,
                    agent,
                    metric,
                    ..
                } => {
                    respond.r(Ok(handle_put_metric_datum(
                        kdirect.clone(),
                        lhnd.clone(),
                        agent,
                        metric,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::QueryMetrics { respond, query, .. } => {
                    respond.r(Ok(handle_query_metrics(
                        kdirect.clone(),
                        lhnd.clone(),
                        query,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::Call {
                    respond,
                    space,
                    to_agent,
                    from_agent,
                    payload,
                    ..
                } => {
                    respond.r(Ok(handle_call(
                        kdirect.clone(),
                        lhnd.clone(),
                        space,
                        to_agent,
                        from_agent,
                        payload,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::Notify { .. } => {
                    unimplemented!()
                }
                event::KitsuneP2pEvent::Gossip {
                    respond,
                    space,
                    to_agent,
                    from_agent,
                    op_hash,
                    op_data,
                    ..
                } => {
                    respond.r(Ok(handle_gossip(
                        kdirect.clone(),
                        lhnd.clone(),
                        space,
                        to_agent,
                        from_agent,
                        op_hash,
                        op_data,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::FetchOpHashesForConstraints { respond, input, .. } => {
                    respond.r(Ok(handle_fetch_op_hashes_for_constraints(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::FetchOpHashData { respond, input, .. } => {
                    respond.r(Ok(handle_fetch_op_hash_data(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::SignNetworkData { respond, input, .. } => {
                    respond.r(Ok(handle_sign_network_data(
                        kdirect.clone(),
                        lhnd.clone(),
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
            }
        },
    )
    .await;
}

async fn handle_put_agent_info_signed(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: PutAgentInfoSignedEvt,
) -> KitsuneResult<()> {
    let PutAgentInfoSignedEvt {
        agent_info_signed, ..
    } = input;

    let agent_info = KdAgentInfo::new(agent_info_signed)?;

    kdirect.persist.store_agent_info(agent_info).await?;

    Ok(())
}

async fn handle_get_agent_info_signed(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: GetAgentInfoSignedEvt,
) -> KitsuneResult<Option<AgentInfoSigned>> {
    let GetAgentInfoSignedEvt { space, agent } = input;

    let root = space.into();
    let agent = agent.into();

    Ok(match kdirect.persist.get_agent_info(root, agent).await {
        Ok(i) => Some(i.into()),
        Err(_) => None,
    })
}

async fn handle_query_agent_info_signed(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: QueryAgentInfoSignedEvt,
) -> KitsuneResult<Vec<AgentInfoSigned>> {
    let QueryAgentInfoSignedEvt { space, .. } = input;

    let root = space.into();

    let map = kdirect.persist.query_agent_info(root).await?;
    Ok(map.into_iter().map(|a| a.into()).collect())
}

async fn handle_put_metric_datum(
    _clone_1: Arc<Kd1>,
    _clone_2: LogicChanHandle<KitsuneDirectEvt>,
    _agent: Arc<KitsuneAgent>,
    _metric: MetricKind,
) -> KitsuneResult<()> {
    todo!()
}

async fn handle_query_metrics(
    _clone_1: Arc<Kd1>,
    _clone_2: LogicChanHandle<KitsuneDirectEvt>,
    _query: MetricQuery,
) -> KitsuneResult<MetricQueryAnswer> {
    todo!()
}

async fn handle_call(
    _kdirect: Arc<Kd1>,
    lhnd: LogicChanHandle<KitsuneDirectEvt>,
    space: Arc<KitsuneSpace>,
    to_agent: Arc<KitsuneAgent>,
    from_agent: Arc<KitsuneAgent>,
    payload: Vec<u8>,
) -> KitsuneResult<Vec<u8>> {
    let root = space.into();
    let to_agent = to_agent.into();
    let from_agent = from_agent.into();

    let (t, content): (String, serde_json::Value) =
        serde_json::from_slice(&payload).map_err(KitsuneError::other)?;
    if t != "message" {
        return Err(format!("unknown call type: {}", t).into());
    }

    let msg = KitsuneDirectEvt::Message {
        root,
        from_agent,
        to_agent,
        content,
    };

    lhnd.emit(msg).await?;

    Ok(b"success".to_vec())
}

async fn handle_gossip(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    space: Arc<KitsuneSpace>,
    to_agent: Arc<KitsuneAgent>,
    _from_agent: Arc<KitsuneAgent>,
    op_hash: Arc<KitsuneOpHash>,
    op_data: Vec<u8>,
) -> KitsuneResult<()> {
    let entry = KdEntry::decode_checked(&op_data).await?;
    let op_hash: KdHash = op_hash.into();
    if &op_hash != entry.hash() {
        return Err("data did not hash to given hash".into());
    }
    let root = space.into();
    let to_agent = to_agent.into();

    kdirect.persist.store_entry(root, to_agent, entry).await?;

    Ok(())
}

async fn handle_fetch_op_hashes_for_constraints(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: FetchOpHashesForConstraintsEvt,
) -> KitsuneResult<Vec<Arc<KitsuneOpHash>>> {
    let FetchOpHashesForConstraintsEvt {
        space,
        agent,
        dht_arc,
        since_utc_epoch_s,
        until_utc_epoch_s,
        ..
    } = input;

    let root = space.into();
    let agent = agent.into();
    let c_start = since_utc_epoch_s as f32;
    let c_end = until_utc_epoch_s as f32;

    // TODO - it's ok for now to just get the full entries
    //        since they'll just get Arc::clone-d
    //        but once this is a persisted database
    //        we'll want an api to just get the hashes
    let entries = kdirect
        .persist
        .query_entries(root, agent, c_start, c_end, dht_arc)
        .await?;

    Ok(entries
        .into_iter()
        .map(|e| e.hash().clone().into())
        .collect())
}

async fn handle_fetch_op_hash_data(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: FetchOpHashDataEvt,
) -> KitsuneResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    let FetchOpHashDataEvt {
        space,
        agent,
        op_hashes,
        ..
    } = input;

    let root: KdHash = space.into();
    let agent: KdHash = agent.into();

    let mut out = Vec::new();

    for op_hash in op_hashes {
        let hash = (&op_hash).into();
        if let Ok(entry) = kdirect
            .persist
            .get_entry(root.clone(), agent.clone(), hash)
            .await
        {
            out.push((op_hash, entry.encode().to_vec()));
        }
    }

    Ok(out)
}

async fn handle_sign_network_data(
    kdirect: Arc<Kd1>,
    _lhnd: LogicChanHandle<KitsuneDirectEvt>,
    input: SignNetworkDataEvt,
) -> KitsuneResult<KitsuneSignature> {
    let SignNetworkDataEvt { agent, data, .. } = input;

    let agent = agent.into();

    let sig = kdirect.persist.sign(agent, &data).await?;
    Ok(KitsuneSignature(sig.to_vec()))
}
