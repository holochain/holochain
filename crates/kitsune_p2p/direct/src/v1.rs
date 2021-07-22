#![allow(clippy::field_reassign_with_default)]
use crate::prelude::*;
use crate::*;

use futures::future::{BoxFuture, FutureExt};
use futures::stream::StreamExt;
use ghost_actor::GhostControlSender;
//use ghost_actor::dependencies::tracing;
use crate::types::direct::*;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p::event::*;
use kitsune_p2p::*;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::ghost_actor;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::HashSet;
use std::future::Future;

/// Config for v1 impl of KitsuneDirect
pub struct KitsuneDirectV1Config {
    /// tuning params
    pub tuning_params: KitsuneP2pTuningParams,

    /// persistence module to use for this kdirect instance
    pub persist: KdPersist,

    /// v1 requires a bootstrap server
    /// specify the addr here
    pub bootstrap: TxUrl,

    /// v1 is only set up to run through a proxy
    /// specify the proxy addr here
    pub proxy: TxUrl,

    /// the localhost port to run the control websocket / ui server on
    pub ui_port: u16,
}

/// Close callback for quick_proxy
pub type CloseCb = Box<dyn FnOnce(u32, &str) -> BoxFuture<'static, ()> + 'static + Send>;

/// run a v1 quick bootstrap instance, returning the url
pub async fn new_quick_bootstrap_v1(
    _tuning_params: KitsuneP2pTuningParams,
) -> KdResult<(TxUrl, KitsuneDirectDriver, CloseCb)> {
    let (driver, addr) = kitsune_p2p_bootstrap::run(([0, 0, 0, 0], 0))
        .await
        .map_err(KdError::other)?;

    let close_cb: CloseCb = Box::new(|_code, _reason| {
        async move {
            // TODO - figure out how to shut down bootstrap server
        }
        .boxed()
    });

    let mut url = url2::url2!("http://{}", addr);

    if let Some(host) = url.host_str() {
        if host == "0.0.0.0" {
            for iface in if_addrs::get_if_addrs().map_err(KdError::other)? {
                // super naive - just picking the first v4 that is not 127.0.0.1
                let addr = iface.addr.ip();
                if let std::net::IpAddr::V4(addr) = addr {
                    if addr != std::net::Ipv4Addr::from([127, 0, 0, 1]) {
                        url.set_host(Some(&iface.addr.ip().to_string())).unwrap();
                        break;
                    }
                }
            }
        }
    }

    Ok((url.into(), driver, close_cb))
}

/// run a v1 quick proxy instance, returning the url
pub async fn new_quick_proxy_v1(
    tuning_params: KitsuneP2pTuningParams,
) -> KdResult<(TxUrl, KitsuneDirectDriver, CloseCb)> {
    use crate::dependencies::*;
    use kitsune_p2p_proxy::tx2::*;
    use kitsune_p2p_transport_quic::tx2::*;
    use kitsune_p2p_types::tls::*;
    use kitsune_p2p_types::tx2::tx2_pool_promote::*;

    let p_tls = TlsConfig::new_ephemeral().await.map_err(KdError::other)?;
    let mut conf = QuicConfig::default();
    conf.tls = Some(p_tls.clone());
    conf.tuning_params = Some(tuning_params.clone());

    let f = QuicBackendAdapt::new(conf).await.map_err(KdError::other)?;
    let f = tx2_pool_promote(f, tuning_params.clone());
    let mut conf = ProxyConfig::default();
    conf.tuning_params = Some(tuning_params.clone());
    conf.allow_proxy_fwd = true;
    let f = tx2_proxy(f, conf).map_err(KdError::other)?;

    let mut proxy = f
        .bind(
            "kitsune-quic://0.0.0.0:0".into(),
            tuning_params.implicit_timeout(),
        )
        .await
        .map_err(KdError::other)?;

    let hnd = proxy.handle().clone();

    let proxy_url = hnd.local_addr().map_err(KdError::other)?;

    let close_cb: CloseCb = Box::new(move |code, reason| hnd.close(code, reason));

    let driver = async move { while proxy.next().await.is_some() {} }.boxed();

    Ok((proxy_url, driver, close_cb))
}

/// create a new v1 instance of the kitsune direct api
#[allow(clippy::manual_async_fn)] // david.b - we have some problems with this
                                  //           future not ending up Send
                                  //           specifying it directly makes
                                  //           for better compile errors
                                  //           when this happens
pub fn new_kitsune_direct_v1(
    conf: KitsuneDirectV1Config,
) -> impl Future<Output = KdResult<(KitsuneDirect, KitsuneDirectDriver)>> + 'static + Send {
    async move {
        let KitsuneDirectV1Config {
            tuning_params,
            persist,
            bootstrap,
            proxy,
            ui_port,
        } = conf;

        let mut sub_config = KitsuneP2pConfig::default();
        sub_config.tuning_params = tuning_params.clone();

        sub_config.bootstrap_service = Some(bootstrap.into());

        sub_config.transport_pool.push(TransportConfig::Proxy {
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

        let (p2p, evt) = spawn_kitsune_p2p(sub_config, tls)
            .await
            .map_err(KdError::other)?;

        let mut logic_chan = <LogicChan<()>>::new(tuning_params.concurrent_limit_per_thread);

        let (srv, srv_evt) = new_srv(Default::default(), ui_port).await?;
        let kdirect = Kd1::new(srv.clone(), persist, p2p);

        let cc = logic_chan.handle().clone();

        cc.capture_logic(handle_events(tuning_params.clone(), kdirect.clone(), evt))
            .await
            .map_err(KdError::other)?;

        cc.capture_logic(handle_srv_events(
            tuning_params,
            kdirect.clone(),
            srv,
            srv_evt,
        ))
        .await
        .map_err(KdError::other)?;

        let kdirect = KitsuneDirect(kdirect);
        let driver = async move { while logic_chan.next().await.is_some() {} }.boxed();

        Ok((kdirect, driver))
    }
}

// -- private -- //

struct Kd1Inner {
    srv: KdSrv,
    p2p: ghost_actor::GhostSender<actor::KitsuneP2p>,
    auth_set: HashSet<Uniq>,
}

#[derive(Clone)]
struct Kd1 {
    uniq: Uniq,
    persist: KdPersist,
    inner: Share<Kd1Inner>,
}

impl Kd1 {
    pub fn new(
        srv: KdSrv,
        persist: KdPersist,
        p2p: ghost_actor::GhostSender<actor::KitsuneP2p>,
    ) -> Arc<Self> {
        Arc::new(Self {
            uniq: Uniq::default(),
            persist,
            inner: Share::new(Kd1Inner {
                srv,
                p2p,
                auth_set: HashSet::new(),
            }),
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
            Ok((i.srv.clone(), i.p2p.clone()))
        });
        async move {
            if let Ok((srv, p2p)) = r {
                srv.close().await;
                let _ = p2p.ghost_actor_shutdown_immediate().await;
            }
        }
        .boxed()
    }

    fn get_persist(&self) -> KdPersist {
        self.persist.clone()
    }

    fn get_ui_addr(&self) -> KdResult<std::net::SocketAddr> {
        self.inner
            .share_mut(|i, _| i.srv.local_addr().map_err(KitsuneError::other))
            .map_err(KdError::other)
    }

    fn list_transport_bindings(&self) -> BoxFuture<'static, KdResult<Vec<TxUrl>>> {
        let fut = self
            .inner
            .share_mut(|i, _| Ok(i.p2p.list_transport_bindings()));
        async move {
            let res = fut.map_err(KdError::other)?.await.map_err(KdError::other)?;
            Ok(res.into_iter().map(|u| u.into()).collect())
        }
        .boxed()
    }

    fn bind_control_handle(&self) -> BoxFuture<'static, KdResult<(KdHnd, KdHndEvtStream)>> {
        let ws_addr = self.get_ui_addr().unwrap();

        // TODO - this should also be configured in v1 conf
        let pass = sodoken::Buffer::new_memlocked(4).unwrap();
        pass.write_lock().copy_from_slice(&[1, 2, 3, 4]);

        async move { new_handle_ws(ws_addr, pass).await.map_err(KdError::other) }.boxed()
    }
}

async fn handle_srv_events(
    tuning_params: KitsuneP2pTuningParams,
    kdirect: Arc<Kd1>,
    srv: KdSrv,
    srv_evt: KdSrvEvtStream,
) {
    let tuning_params = &tuning_params;
    let kdirect = &kdirect;
    let srv = &srv;

    srv_evt
        .for_each_concurrent(
            tuning_params.concurrent_limit_per_thread,
            move |evt| async move {
                match evt {
                    KdSrvEvt::HttpRequest {
                        uri, respond_cb, ..
                    } => {
                        // for now just echoing the incoming uri
                        let r = async move {
                            let (mime, data) = match kdirect.persist.get_ui_file(&uri).await {
                                Ok(r) => r,
                                Err(e) => {
                                    let mut r = HttpResponse::default();
                                    r.status = 500;
                                    r.body = format!("{:?}", e).into_bytes();
                                    return Ok(r);
                                }
                            };
                            let mut r = HttpResponse::default();
                            r.headers.clear();
                            r.headers
                                .push(("Content-Type".to_string(), mime.into_bytes()));
                            r.body = data;
                            Ok(r)
                        }
                        .await;
                        if let Err(err) = respond_cb(r).await {
                            tracing::error!(?err, "http respond error");
                        }
                    }
                    KdSrvEvt::WebsocketConnected { con } => {
                        if let Err(err) = srv.websocket_send(con, KdApi::HelloReq {
                            msg_id: "".to_string(),
                            salt: vec![1, 2, 3, 4].into_boxed_slice().into(),
                        }).await {
                            tracing::error!(?err, "ws send error");
                        }
                    }
                    KdSrvEvt::WebsocketMessage { con, data } => {
                        let msg_id = data.msg_id().to_string();
                        if let KdApi::HelloRes { .. } = data {
                            let _ = kdirect.inner.share_mut(|i, _| {
                                i.auth_set.insert(con);
                                Ok(())
                            });
                            return;
                        }
                        match kdirect.inner.share_mut(|i, _| {
                            Ok(i.auth_set.contains(&con))
                        }) {
                            Ok(true) => (),
                            _ => {
                                if let Err(err) = srv.websocket_send(con, KdApi::ErrorRes {
                                    msg_id,
                                    reason: "unauthenticated".to_string(),
                                }).await {
                                    tracing::error!(?err, "ws send error");
                                }
                                return;
                            }
                        }
                        let exec = |msg_id, fut| async {
                            let res: KdResult<KdApi> = fut.await;
                            let api = match res {
                                Ok(api) => api,
                                Err(err) => {
                                    let reason = format!("{:?}", err);
                                    KdApi::ErrorRes {
                                        msg_id,
                                        reason,
                                    }
                                }
                            };
                            if let Err(err) = srv.websocket_send(con, api).await {
                                tracing::error!(?err, "ws send error");
                            }
                        };
                        match data {
                            KdApi::HelloRes { .. } => unreachable!(),
                            KdApi::User { user } => {
                                tracing::debug!(?user, "recv user data");
                            }
                            KdApi::KeypairGetOrCreateTaggedReq {
                                msg_id,
                                tag: _,
                                ..
                            } => {
                                // TODO - tagging!!!
                                exec(msg_id.clone(), async {
                                    let pub_key = kdirect.persist.generate_signing_keypair().await.map_err(KdError::other)?;
                                    Ok(KdApi::KeypairGetOrCreateTaggedRes {
                                        msg_id,
                                        pub_key,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::AppJoinReq {
                                msg_id,
                                root,
                                agent,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    kdirect.inner.share_mut(|i, _| {
                                        Ok(i.p2p.join(root.to_kitsune_space(), agent.to_kitsune_agent()))
                                    }).map_err(KdError::other)?.await.map_err(KdError::other)?;
                                    Ok(KdApi::AppJoinRes {
                                        msg_id,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::AppLeaveReq {
                                msg_id,
                                root,
                                agent,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    kdirect.inner.share_mut(|i, _| {
                                        Ok(i.p2p.leave(root.to_kitsune_space(), agent.to_kitsune_agent()))
                                    }).map_err(KdError::other)?.await.map_err(KdError::other)?;
                                    Ok(KdApi::AppLeaveRes {
                                        msg_id,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::AgentInfoStoreReq {
                                msg_id,
                                agent_info,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    kdirect.persist.store_agent_info(agent_info).await.map_err(KdError::other)?;
                                    Ok(KdApi::AgentInfoStoreRes {
                                        msg_id,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::AgentInfoGetReq {
                                msg_id,
                                root,
                                agent,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    let agent_info = kdirect.persist.get_agent_info(root, agent).await.map_err(KdError::other)?;
                                    Ok(KdApi::AgentInfoGetRes {
                                        msg_id,
                                        agent_info,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::AgentInfoQueryReq {
                                msg_id,
                                root,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    let agent_info_list = kdirect.persist.query_agent_info(root).await.map_err(KdError::other)?;
                                    Ok(KdApi::AgentInfoQueryRes {
                                        msg_id,
                                        agent_info_list,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::MessageSendReq {
                                msg_id,
                                root,
                                to_agent,
                                from_agent,
                                content,
                                binary,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    let space = root.to_kitsune_space();
                                    let to_agent = to_agent.to_kitsune_agent();
                                    let from_agent = from_agent.to_kitsune_agent();
                                    let content = content.to_string().into_bytes();
                                    let mut payload = Vec::with_capacity(4 + content.len() + binary.len());
                                    let binary_len = (binary.len() as u32).to_le_bytes();
                                    payload.extend_from_slice(&binary_len);
                                    payload.extend_from_slice(&binary);
                                    payload.extend_from_slice(&content);
                                    let res = kdirect.inner.share_mut(move |i, _| {
                                        Ok(i.p2p.rpc_single(space, to_agent, from_agent, payload, None))
                                    }).map_err(KdError::other)?.await.map_err(KdError::other)?;
                                    if res != b"success" {
                                        return Err(format!("unexpected: {}", String::from_utf8_lossy(&res)).into());
                                    }
                                    Ok(KdApi::MessageSendRes {
                                        msg_id,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::EntryAuthorReq {
                                msg_id,
                                root,
                                author,
                                content,
                                binary,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    if author != content.author {
                                        return Err("author mismatch".into());
                                    }
                                    let entry_signed = KdEntrySigned::from_content_with_binary(&kdirect.persist, content, &binary).await?;

                                    // first, put this in our store
                                    // so it can begin gossiping
                                    kdirect.persist.store_entry(root.clone(), author, entry_signed.clone()).await.map_err(KdError::other)?;

                                    // next, let's try to publish it
                                    //
                                    // TODO - make a publish queue
                                    //        so we don't blow out memory
                                    //        spawning all these tasks!
                                    //
                                    //        we don't want to do this inline
                                    //        because in the not connected
                                    //        case, it'll take 30 seconds...
                                    let basis = entry_signed.hash().to_kitsune_basis();
                                    let timeout = tuning_params.implicit_timeout();
                                    let payload = entry_signed.as_wire_data_ref().to_vec();
                                    let fut = kdirect.inner.share_mut(|i, _| {
                                        Ok(i.p2p.broadcast(
                                            root.to_kitsune_space(),
                                            basis,
                                            timeout,
                                            payload,
                                        ))
                                    }).map_err(KdError::other)?;
                                    tokio::task::spawn(async move {
                                        if let Err(err) = fut.await.map_err(KdError::other) {
                                            tracing::warn!(?err, "publish error");
                                        }
                                    });

                                    Ok(KdApi::EntryAuthorRes {
                                        msg_id,
                                        entry_signed,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::EntryGetReq {
                                msg_id,
                                root,
                                agent,
                                hash,
                                ..
                            } => {
                                exec(msg_id.clone(), async {
                                    let entry_signed = kdirect.persist.get_entry(root, agent, hash).await.map_err(KdError::other)?;
                                    Ok(KdApi::EntryGetRes {
                                        msg_id,
                                        entry_signed,
                                    })
                                }.boxed()).await;
                            }
                            KdApi::EntryGetChildrenReq {
                                //msg_id,
                                //root,
                                //parent,
                                //kind,
                                ..
                            } => {
                                // TODO -- FIXME
                                unimplemented!("TODO")
                            }
                            oth @ KdApi::ErrorRes { .. } |
                            oth @ KdApi::HelloReq { .. } |
                            oth @ KdApi::KeypairGetOrCreateTaggedRes { .. } |
                            oth @ KdApi::AppJoinRes { .. } |
                            oth @ KdApi::AppLeaveRes { .. } |
                            oth @ KdApi::AgentInfoStoreRes { .. } |
                            oth @ KdApi::AgentInfoGetRes { .. } |
                            oth @ KdApi::AgentInfoQueryRes { .. } |
                            oth @ KdApi::MessageSendRes { .. } |
                            oth @ KdApi::MessageRecvEvt { .. } |
                            oth @ KdApi::EntryAuthorRes { .. } |
                            oth @ KdApi::EntryGetRes { .. } |
                            oth @ KdApi::EntryGetChildrenRes { .. } => {
                                let reason = format!("unexpected {}", oth);
                                if let Err(err) = srv.websocket_send(con, KdApi::ErrorRes {
                                    msg_id,
                                    reason,
                                }).await {
                                    tracing::error!(?err, "ws send error");
                                }
                            }
                        }
                    }
                }
            },
        )
        .await;
}

async fn handle_events(
    tuning_params: KitsuneP2pTuningParams,
    kdirect: Arc<Kd1>,
    evt: futures::channel::mpsc::Receiver<event::KitsuneP2pEvent>,
) {
    use futures::future::TryFutureExt;
    let kdirect = &kdirect;

    evt.for_each_concurrent(
        tuning_params.concurrent_limit_per_thread,
        move |evt| async move {
            match evt {
                event::KitsuneP2pEvent::PutAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_put_agent_info_signed(kdirect.clone(), input)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::GetAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_get_agent_info_signed(kdirect.clone(), input)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::QueryAgentInfoSigned { respond, input, .. } => {
                    respond.r(Ok(handle_query_agent_info_signed(kdirect.clone(), input)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::QueryGossipAgents { respond, input, .. } => {
                    respond.r(Ok(handle_query_gossip_agents(kdirect.clone(), input)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::QueryAgentInfoSignedNearBasis {
                    respond,
                    space,
                    basis_loc,
                    limit,
                    ..
                } => {
                    respond.r(Ok(handle_query_agent_info_signed_near_basis(
                        kdirect.clone(),
                        space,
                        basis_loc,
                        limit,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::PutMetricDatum { respond, datum, .. } => {
                    respond.r(Ok(handle_put_metric_datum(kdirect.clone(), datum)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::QueryMetrics { respond, query, .. } => {
                    respond.r(Ok(handle_query_metrics(kdirect.clone(), query)
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
                        space,
                        to_agent,
                        from_agent,
                        payload,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::Notify {
                    respond,
                    space,
                    to_agent,
                    payload,
                    ..
                } => {
                    let kdirect = kdirect.clone();
                    respond.r(Ok(async move {
                        let entry = KdEntrySigned::from_wire(payload.into())
                            .await
                            .map_err(KitsuneP2pError::other)?;
                        let root = KdHash::from_kitsune_space(&space);
                        let to_agent = KdHash::from_kitsune_agent(&to_agent);
                        kdirect
                            .persist
                            .store_entry(root, to_agent, entry)
                            .await
                            .map_err(KitsuneP2pError::other)
                    }
                    .boxed()
                    .into()));
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
                        input,
                    )
                    .map_err(KitsuneP2pError::other)
                    .boxed()
                    .into()));
                }
                event::KitsuneP2pEvent::FetchOpHashData { respond, input, .. } => {
                    respond.r(Ok(handle_fetch_op_hash_data(kdirect.clone(), input)
                        .map_err(KitsuneP2pError::other)
                        .boxed()
                        .into()));
                }
                event::KitsuneP2pEvent::SignNetworkData { respond, input, .. } => {
                    respond.r(Ok(handle_sign_network_data(kdirect.clone(), input)
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
    input: PutAgentInfoSignedEvt,
) -> KdResult<()> {
    let PutAgentInfoSignedEvt {
        agent_info_signed, ..
    } = input;

    let agent_info = KdAgentInfo::from_kitsune(&agent_info_signed)?;

    kdirect.persist.store_agent_info(agent_info).await?;

    Ok(())
}

async fn handle_get_agent_info_signed(
    kdirect: Arc<Kd1>,
    input: GetAgentInfoSignedEvt,
) -> KdResult<Option<AgentInfoSigned>> {
    let GetAgentInfoSignedEvt { space, agent } = input;

    let root = KdHash::from_kitsune_space(&space);
    let agent = KdHash::from_kitsune_agent(&agent);

    Ok(match kdirect.persist.get_agent_info(root, agent).await {
        Ok(i) => Some(i.to_kitsune()),
        Err(_) => None,
    })
}

async fn handle_query_gossip_agents(
    _kdirect: Arc<Kd1>,
    _input: kitsune_p2p::event::QueryGossipAgentsEvt,
) -> KdResult<
    Vec<(
        Arc<kitsune_p2p::KitsuneAgent>,
        kitsune_p2p_types::dht_arc::ArcInterval,
    )>,
> {
    todo!()
}

async fn handle_query_agent_info_signed(
    kdirect: Arc<Kd1>,
    input: QueryAgentInfoSignedEvt,
) -> KdResult<Vec<AgentInfoSigned>> {
    let QueryAgentInfoSignedEvt { space, .. } = input;

    let root = KdHash::from_kitsune_space(&space);

    let map = kdirect.persist.query_agent_info(root).await?;
    Ok(map.into_iter().map(|a| a.to_kitsune()).collect())
}

async fn handle_query_agent_info_signed_near_basis(
    kdirect: Arc<Kd1>,
    space: Arc<KitsuneSpace>,
    basis_loc: u32,
    limit: u32,
) -> KdResult<Vec<AgentInfoSigned>> {
    let root = KdHash::from_kitsune_space(&space);
    let map = kdirect
        .persist
        .query_agent_info_near_basis(root, basis_loc, limit)
        .await?;
    Ok(map.into_iter().map(|a| a.to_kitsune()).collect())
}

async fn handle_put_metric_datum(kdirect: Arc<Kd1>, datum: MetricDatum) -> KdResult<()> {
    kdirect.persist.store_metric_datum(datum).await
}

async fn handle_query_metrics(
    kdirect: Arc<Kd1>,
    query: MetricQuery,
) -> KdResult<MetricQueryAnswer> {
    // Why are there two nested futures here?
    kdirect.persist.fetch_metrics(query).await.await
}

async fn handle_call(
    kdirect: Arc<Kd1>,
    space: Arc<KitsuneSpace>,
    to_agent: Arc<KitsuneAgent>,
    from_agent: Arc<KitsuneAgent>,
    payload: Vec<u8>,
) -> KdResult<Vec<u8>> {
    let root = KdHash::from_kitsune_space(&space);
    let to_agent = KdHash::from_kitsune_agent(&to_agent);
    let from_agent = KdHash::from_kitsune_agent(&from_agent);

    if payload.len() < 4 {
        return Err(format!("invalid msg size: {}", payload.len()).into());
    }

    let binary_len = u32::from_le_bytes(*arrayref::array_ref![&payload, 0, 4]) as usize;

    if payload.len() < 4 + binary_len {
        return Err(format!(
            "invalid msg size: {} (binary_len: {})",
            payload.len(),
            binary_len
        )
        .into());
    }

    use kitsune_p2p_direct_api::kd_entry::KdEntryBinary;
    let binary: KdEntryBinary = payload[4..4 + binary_len]
        .to_vec()
        .into_boxed_slice()
        .into();

    let content: serde_json::Value =
        serde_json::from_slice(&payload[4 + binary_len..]).map_err(KdError::other)?;

    kdirect
        .inner
        .share_mut(move |i, _| {
            Ok(i.srv.websocket_broadcast(KdApi::MessageRecvEvt {
                root,
                to_agent,
                from_agent,
                content,
                binary,
            }))
        })
        .map_err(KdError::other)?
        .await?;

    Ok(b"success".to_vec())
}

async fn handle_gossip(
    kdirect: Arc<Kd1>,
    space: Arc<KitsuneSpace>,
    to_agent: Arc<KitsuneAgent>,
    _from_agent: Arc<KitsuneAgent>,
    op_hash: Arc<KitsuneOpHash>,
    op_data: Vec<u8>,
) -> KdResult<()> {
    let entry = KdEntrySigned::from_wire(op_data.into_boxed_slice())
        .await
        .map_err(KdError::other)?;
    let op_hash = KdHash::from_kitsune_op_hash(&op_hash);
    if &op_hash != entry.hash() {
        return Err("data did not hash to given hash".into());
    }
    let root = KdHash::from_kitsune_space(&space);
    let to_agent = KdHash::from_kitsune_agent(&to_agent);

    kdirect.persist.store_entry(root, to_agent, entry).await?;

    Ok(())
}

#[allow(warnings)]
async fn handle_fetch_op_hashes_for_constraints(
    kdirect: Arc<Kd1>,
    input: FetchOpHashesForConstraintsEvt,
) -> KdResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
    let FetchOpHashesForConstraintsEvt {
        space,
        agents,
        window,
        max_ops,
        ..
    } = input;

    todo!(
        "reimplement for new api,
        in particular making sure the time window lines up"
    );

    // let root = KdHash::from_kitsune_space(&space);
    // let agent = KdHash::from_kitsune_agent(&agent);
    // let c_start = since_utc_epoch_s as f32;
    // let c_end = until_utc_epoch_s as f32;

    // // TODO - it's ok for now to just get the full entries
    // //        since they'll just get Arc::clone-d
    // //        but once this is a persisted database
    // //        we'll want an api to just get the hashes
    // let entries = kdirect
    //     .persist
    //     .query_entries(root, agent, c_start, c_end, dht_arc)
    //     .await?;

    // Ok(entries
    //     .into_iter()
    //     .map(|e| e.hash().clone().to_kitsune_op_hash())
    //     .collect())
}

async fn handle_fetch_op_hash_data(
    kdirect: Arc<Kd1>,
    input: FetchOpHashDataEvt,
) -> KdResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    let FetchOpHashDataEvt {
        space,
        agents,
        op_hashes,
        ..
    } = input;

    let mut out = Vec::new();
    let root = KdHash::from_kitsune_space(&space);

    for op_hash in op_hashes {
        for agent in agents.iter() {
            let agent = KdHash::from_kitsune_agent(agent);
            let hash = KdHash::from_kitsune_op_hash(&op_hash);
            if let Ok(entry) = kdirect
                .persist
                .get_entry(root.clone(), agent.clone(), hash)
                .await
            {
                out.push((op_hash.clone(), entry.as_wire_data_ref().to_vec()));
            }
        }
    }

    Ok(out)
}

async fn handle_sign_network_data(
    kdirect: Arc<Kd1>,
    input: SignNetworkDataEvt,
) -> KdResult<KitsuneSignature> {
    let SignNetworkDataEvt { agent, data, .. } = input;

    let agent = KdHash::from_kitsune_agent(&agent);

    let sig = kdirect.persist.sign(agent, &data).await?;
    Ok(KitsuneSignature(sig.to_vec()))
}
