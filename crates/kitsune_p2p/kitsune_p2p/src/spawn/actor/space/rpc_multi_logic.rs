use super::*;
use futures::future::BoxFuture;

pub(crate) async fn handle_rpc_multi(
    input: actor::RpcMulti,
    ro_inner: Arc<SpaceReadOnlyInner>,
    local_joined_agents: HashSet<Arc<KitsuneAgent>>,
) -> KitsuneP2pResult<Vec<actor::RpcMultiResponse>> {
    handle_rpc_multi_as_single(input, ro_inner, local_joined_agents).await
}

pub(crate) async fn handle_rpc_multi_as_single(
    input: actor::RpcMulti,
    ro_inner: Arc<SpaceReadOnlyInner>,
    local_joined_agents: HashSet<Arc<KitsuneAgent>>,
) -> KitsuneP2pResult<Vec<actor::RpcMultiResponse>> {
    let RpcMulti {
        space,
        basis,
        payload,
        max_timeout,
        ..
    } = input;

    let ro_inner = &ro_inner;
    let space = &space;
    let payload = &payload;

    let make_req = move |con_hnd: Tx2ConHnd<crate::wire::Wire>,
                         agent: Arc<KitsuneAgent>|
          -> BoxFuture<'_, KitsuneP2pResult<Vec<actor::RpcMultiResponse>>> {
        async move {
            let msg = wire::Wire::call(space.clone(), agent.clone(), payload.clone().into());

            let start = tokio::time::Instant::now();

            let res = con_hnd.request(&msg, max_timeout).await;

            match res {
                Ok(wire::Wire::CallResp(c)) => {
                    ro_inner
                        .metrics
                        .write()
                        .record_reachability_event(true, [&agent]);
                    ro_inner
                        .metrics
                        .write()
                        .record_latency_micros(start.elapsed().as_micros(), [&agent]);
                    Ok(vec![RpcMultiResponse {
                        agent: agent.clone(),
                        response: c.data.into(),
                    }])
                }
                oth => {
                    ro_inner
                        .metrics
                        .write()
                        .record_reachability_event(false, [&agent]);
                    ro_inner
                        .metrics
                        .write()
                        .record_latency_micros(start.elapsed().as_micros(), [&agent]);
                    tracing::warn!(?oth, "unexpected remote call result");
                    Err(format!("rpc_multi request failed: {:?}", oth).into())
                }
            }
        }
        .boxed()
    };

    max_timeout
        .mix("rpc_multi", async move {
            let mut errs = vec![];
            for _ in 0..2 {
                let mut infos = None;

                if let Ok(i) = discover::get_cached_remotes_near_basis(
                    ro_inner.clone(),
                    basis.get_loc(),
                    max_timeout,
                )
                .await
                {
                    if !i.is_empty() {
                        infos = Some(i);
                    }
                }

                if infos.is_none() {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    if let Ok(i) = discover::get_cached_remotes_near_basis(
                        ro_inner.clone(),
                        basis.get_loc(),
                        max_timeout,
                    )
                    .await
                    {
                        if !i.is_empty() {
                            infos = Some(i);
                        }
                    }
                }

                if let Some(mut infos) = infos {
                    rand::seq::SliceRandom::shuffle(infos.as_mut_slice(), &mut rand::thread_rng());

                    for info in infos {
                        use discover::PeerDiscoverResult;

                        let con_hnd = match discover::peer_connect(
                            ro_inner.clone(),
                            &info,
                            max_timeout,
                        )
                        .await
                        {
                            PeerDiscoverResult::OkShortcut => {
                                tracing::warn!("remote peer is local");
                                continue;
                            }
                            PeerDiscoverResult::Err(err) => {
                                tracing::warn!(?err, "peer discovery error");
                                errs.push(err);
                                continue;
                            }
                            PeerDiscoverResult::OkRemote { con_hnd, .. } => con_hnd,
                        };

                        match make_req(con_hnd, info.agent.clone()).await {
                            Ok(res) => return Ok(res),
                            Err(err) => {
                                tracing::warn!(?err, "remote call error");
                                errs.push(err);
                                continue;
                            }
                        }
                    }
                }
            }

            let num_local_agents = local_joined_agents.len();

            // fall back to self-get
            for agent in local_joined_agents {
                match ro_inner
                    .evt_sender
                    .call(space.clone(), agent.clone(), payload.clone())
                    .await
                {
                    Ok(response) => {
                        return Ok(vec![RpcMultiResponse { agent, response }]);
                    }
                    Err(err) => {
                        tracing::warn!(?err, "local call error");
                        errs.push(err);
                        continue;
                    }
                }
            }

            // finally, return an error
            let error_msg = format!(
                "rpc_multi failed to get results. Local agents: {}, Errors: {:?}",
                num_local_agents, errs
            );
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        })
        .await
        .map_err(|err| err.into())
}
