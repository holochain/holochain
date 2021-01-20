use crate::*;

mod persist;
use persist::*;

mod keystore;
pub use keystore::KdHash;
use keystore::*;

mod wire;
use wire::*;

pub struct KdActorInner {
    persist: Persist,
    keystore: Keystore,
    binding: old_ghost_actor::GhostSender<actor::KitsuneP2p>,
    active: HashMap<KdHash, tokio::sync::mpsc::Sender<KdEvent>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct KdActor(ghost_actor::GhostActor<KdActorInner>);

impl KdActor {
    #[allow(clippy::new_ret_no_self)]
    pub async fn new(config: KdConfig) -> KdResult<KitsuneDirect> {
        let (p2p, evt) = spawn_p2p(&config.directives).await?;

        let persist = spawn_persist_sqlcipher(config).await?;
        let keystore = spawn_keystore(persist.clone());

        let (actor, driver) = ghost_actor::GhostActor::new(KdActorInner {
            persist,
            keystore,
            binding: p2p,
            active: HashMap::new(),
        });
        tokio::task::spawn(driver);

        handle_events(actor.clone(), evt);

        Ok(KitsuneDirect(Box::new(Self(actor))))
    }
}

#[allow(clippy::ptr_arg)]
async fn spawn_p2p(
    config_directives: &Vec<String>,
) -> KdResult<(
    old_ghost_actor::GhostSender<actor::KitsuneP2p>,
    futures::channel::mpsc::Receiver<event::KitsuneP2pEvent>,
)> {
    let mut should_proxy = false;
    let mut bind_mem_local = false;

    for d in config_directives.iter() {
        match &d as &str {
            "set_proxy_accept_all:" => should_proxy = true,
            "bind_mem_local:" => bind_mem_local = true,
            _ => {
                return Err(format!("invalid config directive: {}", d).into());
            }
        }
    }

    let mut config = KitsuneP2pConfig::default();

    let proxy = if should_proxy {
        Some(ProxyAcceptConfig::AcceptAll)
    } else {
        Some(ProxyAcceptConfig::RejectAll)
    };

    if bind_mem_local {
        config.transport_pool.push(TransportConfig::Proxy {
            sub_transport: Box::new(TransportConfig::Mem {}),
            proxy_config: ProxyConfig::LocalProxyServer {
                proxy_accept_config: proxy,
            },
        });
    }

    Ok(spawn_kitsune_p2p(
        config,
        kitsune_p2p_proxy::TlsConfig::new_ephemeral().await.unwrap(),
    )
    .await?)
}

fn handle_events(
    actor: ghost_actor::GhostActor<KdActorInner>,
    evt: futures::channel::mpsc::Receiver<event::KitsuneP2pEvent>,
) {
    tokio::task::spawn(async move {
        futures::stream::StreamExt::for_each_concurrent(evt, 32, move |evt| {
            let actor = actor.clone();
            async move {
                handle_event(actor, evt);
            }
        })
        .await;
    });
}

fn handle_event(actor: ghost_actor::GhostActor<KdActorInner>, evt: event::KitsuneP2pEvent) {
    use event::KitsuneP2pEvent::*;
    match evt {
        PutAgentInfoSigned { respond, input, .. } => {
            respond.respond(handle_put_agent_info_signed(actor, input));
        }
        GetAgentInfoSigned { respond, input, .. } => {
            respond.respond(handle_get_agent_info_signed(actor, input));
        }
        QueryAgentInfoSigned { respond, input, .. } => {
            respond.respond(handle_query_agent_info_signed(actor, input));
        }
        Call {
            respond,
            space,
            to_agent,
            from_agent,
            payload,
            ..
        } => {
            respond.respond(handle_call(actor, space, to_agent, from_agent, payload));
        }
        Notify {
            respond,
            space,
            to_agent,
            from_agent,
            payload,
            ..
        } => {
            respond.respond(handle_notify(actor, space, to_agent, from_agent, payload));
        }
        Gossip {
            respond,
            space,
            to_agent,
            from_agent,
            op_hash,
            op_data,
            ..
        } => {
            respond.respond(handle_gossip(
                actor, space, to_agent, from_agent, op_hash, op_data,
            ));
        }
        FetchOpHashesForConstraints { respond, input, .. } => {
            respond.respond(handle_fetch_op_hashes_for_constraints(actor, input));
        }
        FetchOpHashData { respond, input, .. } => {
            respond.respond(handle_fetch_op_hash_data(actor, input));
        }
        SignNetworkData { respond, input, .. } => {
            respond.respond(handle_sign_network_data(actor, input));
        }
    }
}

fn g2o<R: 'static + Send>(
    g: ghost_actor::GhostFuture<R, KdError>,
) -> event::KitsuneP2pEventHandlerResult<R> {
    Ok(
        async move { Ok(g.await.map_err(kitsune_p2p::KitsuneP2pError::other)?) }
            .boxed()
            .into(),
    )
}

fn handle_put_agent_info_signed(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::PutAgentInfoSignedEvt,
) -> event::KitsuneP2pEventHandlerResult<()> {
    let event::PutAgentInfoSignedEvt {
        space,
        agent: _,
        agent_info_signed,
    } = input;
    let space: KdHash = space.into();
    g2o(ghost_actor::resp(async move {
        actor
            .invoke_async(move |inner| {
                let fut = inner.persist.store_agent_info(space, agent_info_signed);
                Ok(ghost_actor::resp(async move { Ok(fut.await?) }))
            })
            .await
    }))
}

fn handle_get_agent_info_signed(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::GetAgentInfoSignedEvt,
) -> event::KitsuneP2pEventHandlerResult<Option<agent_store::AgentInfoSigned>> {
    let event::GetAgentInfoSignedEvt { space, agent } = input;
    let space: KdHash = space.into();
    let agent: KdHash = agent.into();
    g2o(ghost_actor::resp(async move {
        actor
            .invoke_async(move |inner| {
                let fut = inner.persist.get_agent_info(space, agent);
                Ok(ghost_actor::resp(async move {
                    Ok(match fut.await {
                        Ok(i) => Some(i),
                        Err(_) => None,
                    })
                }))
            })
            .await
    }))
}

fn handle_query_agent_info_signed(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::QueryAgentInfoSignedEvt,
) -> event::KitsuneP2pEventHandlerResult<Vec<agent_store::AgentInfoSigned>> {
    let event::QueryAgentInfoSignedEvt { space, agent: _ } = input;
    let space: KdHash = space.into();
    g2o(ghost_actor::resp(async move {
        actor
            .invoke_async(move |inner| {
                let fut = inner.persist.query_agent_info(space);
                Ok(ghost_actor::resp(async move { fut.await }))
            })
            .await
    }))
}

fn handle_call(
    actor: ghost_actor::GhostActor<KdActorInner>,
    space: Arc<KitsuneSpace>,
    to_agent: Arc<KitsuneAgent>,
    from_agent: Arc<KitsuneAgent>,
    payload: Vec<u8>,
) -> event::KitsuneP2pEventHandlerResult<Vec<u8>> {
    g2o(ghost_actor::resp(async move {
        let to_active_agent = KdHash::from(to_agent);
        let to_active_agent_clone = to_active_agent.clone();
        let mut send = match actor
            .invoke(move |inner| {
                KdResult::Ok(match inner.active.get(&to_active_agent_clone) {
                    Some(send) => Some(send.clone()),
                    None => None,
                })
            })
            .await?
        {
            Some(send) => send,
            None => {
                return Ok(Wire::failure("no active agent".to_string()).encode_vec()?);
            }
        };

        let res: KdResult<()> = async move {
            let root_agent = KdHash::from(space);
            let from_active_agent = KdHash::from(from_agent);
            let (_, content) = Wire::decode_ref(&payload)?;
            let content = match content {
                Wire::Message(Message { content }) => Ok(content),
                _ => Err(KdError::from("invalid message")),
            }?;
            let evt = KdEvent::Message {
                root_agent,
                to_active_agent,
                from_active_agent,
                content,
            };
            send.send(evt).await.map_err(KdError::other)?;
            Ok(())
        }
        .await;

        if let Err(e) = res {
            Ok(Wire::failure(format!("{:?}", e)).encode_vec()?)
        } else {
            Ok(Wire::success().encode_vec()?)
        }
    }))
}

fn handle_notify(
    _actor: ghost_actor::GhostActor<KdActorInner>,
    _space: Arc<KitsuneSpace>,
    _to_agent: Arc<KitsuneAgent>,
    _from_agent: Arc<KitsuneAgent>,
    _payload: Vec<u8>,
) -> event::KitsuneP2pEventHandlerResult<()> {
    unimplemented!()
}

fn handle_gossip(
    actor: ghost_actor::GhostActor<KdActorInner>,
    space: Arc<KitsuneSpace>,
    _to_agent: Arc<KitsuneAgent>,
    _from_agent: Arc<KitsuneAgent>,
    _op_hash: Arc<KitsuneOpHash>,
    op_data: Vec<u8>,
) -> event::KitsuneP2pEventHandlerResult<()> {
    let root_agent: KdHash = space.into();
    g2o(ghost_actor::resp(async move {
        let entry = KdEntry::from_raw_bytes_validated(op_data.into_boxed_slice()).await?;
        actor
            .invoke_async(move |inner| Ok(inner.persist.store_entry(root_agent, entry)))
            .await?;
        Ok(())
    }))
}

fn chrono_from_epoch_s(epoch_s: i64) -> DateTime<Utc> {
    if epoch_s < chrono::MIN_DATETIME.timestamp() {
        return chrono::MIN_DATETIME;
    }
    if epoch_s > chrono::MAX_DATETIME.timestamp() {
        return chrono::MAX_DATETIME;
    }
    DateTime::from_utc(
        chrono::naive::NaiveDateTime::from_timestamp(epoch_s, 0),
        chrono::Utc,
    )
}

fn handle_fetch_op_hashes_for_constraints(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::FetchOpHashesForConstraintsEvt,
) -> event::KitsuneP2pEventHandlerResult<Vec<Arc<KitsuneOpHash>>> {
    let event::FetchOpHashesForConstraintsEvt {
        space,
        agent,
        dht_arc,
        since_utc_epoch_s,
        until_utc_epoch_s,
    } = input;
    let root_agent: KdHash = space.into();
    let _agent: KdHash = agent.into();
    let dht_arc = dht_arc;
    let since: DateTime<Utc> = chrono_from_epoch_s(since_utc_epoch_s);
    let until: DateTime<Utc> = chrono_from_epoch_s(until_utc_epoch_s);
    g2o(ghost_actor::resp(async move {
        actor
            .invoke_async(move |inner| {
                let fut = inner
                    .persist
                    .query_entries(root_agent, since, until, dht_arc);

                Ok(ghost_actor::resp(async move {
                    Ok(fut
                        .await?
                        .into_iter()
                        .map(|entry| {
                            let hash = entry.hash();
                            let hash: KitsuneOpHash = hash.into();
                            Arc::new(hash)
                        })
                        .collect())
                }))
            })
            .await
    }))
}

fn handle_fetch_op_hash_data(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::FetchOpHashDataEvt,
) -> event::KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
    let event::FetchOpHashDataEvt {
        space,
        agent,
        op_hashes,
    } = input;
    let root_agent: KdHash = space.into();
    let _agent: KdHash = agent.into();
    g2o(ghost_actor::resp(async move {
        let mut out = Vec::new();
        for hash in op_hashes {
            let root_agent = root_agent.clone();
            let hash: KdHash = hash.into();
            out.push(
                actor
                    .invoke_async(move |inner| {
                        let fut = inner.persist.get_entry(root_agent, hash);
                        Ok(ghost_actor::resp(async move {
                            let entry = fut.await?;
                            let hash: KitsuneOpHash = entry.hash().into();
                            KdResult::Ok((Arc::new(hash), entry.as_ref().to_vec()))
                        }))
                    })
                    .await?,
            );
        }
        Ok(out)
    }))
}

fn handle_sign_network_data(
    actor: ghost_actor::GhostActor<KdActorInner>,
    input: event::SignNetworkDataEvt,
) -> event::KitsuneP2pEventHandlerResult<KitsuneSignature> {
    let event::SignNetworkDataEvt {
        space: _,
        agent,
        data,
    } = input;
    let agent: KdHash = agent.into();
    let data = sodoken::Buffer::from_ref(&*data);
    g2o(ghost_actor::resp(async move {
        actor
            .invoke_async(move |inner| {
                let fut = inner.keystore.sign(agent, data);

                Ok(ghost_actor::resp(async move {
                    let sig = fut.await?;
                    Ok(KitsuneSignature(sig.to_vec()))
                }))
            })
            .await
    }))
}

impl AsKitsuneDirect for KdActor {
    ghost_actor::ghost_box_trait_impl_fns!(AsKitsuneDirect);

    fn list_transport_bindings(&self) -> ghost_actor::GhostFuture<Vec<Url2>, KdError> {
        self.0.invoke_async(move |inner| {
            let fut = inner.binding.list_transport_bindings();
            Ok(ghost_actor::resp(async move { Ok(fut.await?) }))
        })
    }

    fn generate_agent(&self) -> ghost_actor::GhostFuture<KdHash, KdError> {
        self.0.invoke_async(move |inner| {
            let fut = inner.keystore.generate_sign_agent();
            Ok(ghost_actor::resp(async move { fut.await }))
        })
    }

    fn sign(
        &self,
        pub_key: KdHash,
        data: sodoken::Buffer,
    ) -> ghost_actor::GhostFuture<Arc<[u8; 64]>, KdError> {
        self.0
            .invoke_async(move |inner| Ok(inner.keystore.sign(pub_key, data)))
    }

    fn join(
        &self,
        root_agent: KdHash,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        self.0.invoke_async(move |inner| {
            let fut = inner.binding.join(root_agent.into(), acting_agent.into());
            Ok(ghost_actor::resp(async move { Ok(fut.await?) }))
        })
    }

    fn list_known_agent_info(
        &self,
        root_agent: KdHash,
    ) -> ghost_actor::GhostFuture<Vec<agent_store::AgentInfoSigned>, KdError> {
        self.0
            .invoke_async(move |inner| Ok(inner.persist.query_agent_info(root_agent)))
    }

    fn inject_agent_info(
        &self,
        root_agent: KdHash,
        agent_info: Vec<agent_store::AgentInfoSigned>,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        self.0.invoke_async(move |inner| {
            let mut all = Vec::new();
            for i in agent_info {
                all.push(inner.persist.store_agent_info(root_agent.clone(), i));
            }
            Ok(ghost_actor::resp(async move {
                futures::future::try_join_all(all).await?;
                Ok(())
            }))
        })
    }

    fn activate(
        &self,
        acting_agent: KdHash,
    ) -> ghost_actor::GhostFuture<tokio::sync::mpsc::Receiver<KdEvent>, KdError> {
        self.0.invoke(move |inner| {
            let (send, recv) = tokio::sync::mpsc::channel(32);
            inner.active.insert(acting_agent, send);
            Ok(recv)
        })
    }

    fn message(
        &self,
        root_agent: KdHash,
        from_active_agent: KdHash,
        to_active_agent: KdHash,
        content: serde_json::Value,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        let actor = self.0.clone();
        ghost_actor::resp(async move {
            let msg = Wire::Message(Message { content })
                .encode_vec()
                .map_err(KdError::other)?;

            actor
                .invoke_async(move |inner| {
                    let fut = inner.binding.rpc_single(
                        root_agent.into(),
                        to_active_agent.into(),
                        from_active_agent.into(),
                        msg,
                        None,
                    );

                    Ok(ghost_actor::resp(async move {
                        fut.await?;
                        Ok(())
                    }))
                })
                .await
        })
    }

    fn create_entry(
        &self,
        root_agent: KdHash,
        by_agent: KdHash,
        new_entry: KdEntryBuilder,
    ) -> ghost_actor::GhostFuture<(), KdError> {
        let kd = KitsuneDirect(Box::new(self.clone()));
        let actor = self.0.clone();
        ghost_actor::resp(async move {
            let new_entry = new_entry.build(by_agent, kd).await?;
            actor
                .invoke_async(move |inner| Ok(inner.persist.store_entry(root_agent, new_entry)))
                .await?;
            Ok(())
        })
    }
}
