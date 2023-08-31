use crate::actor::BroadcastData;
use crate::event::{
    FetchOpDataEvt, FetchOpDataEvtQuery, GetAgentInfoSignedEvt, KitsuneP2pEvent,
    KitsuneP2pEventSender, PutAgentInfoSignedEvt, QueryAgentsEvt,
};
use crate::spawn::actor::fetch::FetchResponseConfig;
use crate::spawn::actor::{
    Internal, InternalResult, InternalSender, UNAUTHORIZED_DISCONNECT_CODE,
    UNAUTHORIZED_DISCONNECT_REASON,
};
use crate::spawn::meta_net::{
    nodespace_is_authorized, MetaNetAuth, MetaNetCon, MetaNetEvt, MetaNetEvtRecv,
};
use crate::{wire, HostApi, KitsuneP2pConfig, KitsuneP2pError};
use futures::channel::mpsc::Sender;
use futures::{SinkExt, StreamExt};
use ghost_actor::GhostSender;
use kitsune_p2p_fetch::{FetchKey, FetchPool, FetchResponseQueue};
use kitsune_p2p_timestamp::Timestamp;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::AbortHandle;

pub struct MetaNetTask {
    evt_sender: Sender<KitsuneP2pEvent>,
    host: HostApi,
    config: KitsuneP2pConfig,
    fetch_pool: FetchPool,
    fetch_response_queue: FetchResponseQueue<FetchResponseConfig>,
    ep_evt: Option<MetaNetEvtRecv>,
    i_s: GhostSender<Internal>,
    is_finished: Arc<AtomicBool>,
}

#[derive(thiserror::Error, Debug)]
enum MetaNetTaskError {
    #[error("Ghost actor closed")]
    GhostActorClosed(#[from] ghost_actor::GhostError),

    #[error("This error should be ignored")]
    Ignored,
}

type MetaNetTaskResult<T> = Result<T, MetaNetTaskError>;

impl MetaNetTask {
    pub fn new(
        evt_sender: Sender<KitsuneP2pEvent>,
        host: HostApi,
        config: KitsuneP2pConfig,
        fetch_pool: FetchPool,
        fetch_response_queue: FetchResponseQueue<FetchResponseConfig>,
        ep_evt: MetaNetEvtRecv,
        i_s: GhostSender<Internal>,
    ) -> Self {
        Self {
            evt_sender,
            host,
            config,
            fetch_pool,
            fetch_response_queue,
            ep_evt: Some(ep_evt),
            i_s,
            is_finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn spawn(mut self) {
        // Use an mpsc channel rather than a oneshot so no locking is needed in this code to sync the sender.
        let (shutdown_send, mut shutdown_recv) = futures::channel::mpsc::channel(1);

        let is_finished = self.is_finished.clone();

        let join_handle = tokio::task::spawn({
            let tuning_params = self.config.tuning_params.clone();
            async move {
                let ep_evt = self
                    .ep_evt
                    .take()
                    .expect("There should always be an ep_evt");

                let this = Arc::new(self);

                let ep_evt_run = ep_evt
                    .for_each_concurrent(tuning_params.concurrent_limit_per_thread, move |event| {
                        let evt_sender = this.evt_sender.clone();
                        let host = this.host.clone();
                        let i_s = this.i_s.clone();
                        let mut this = this.clone();
                        let mut shutdown_send = shutdown_send.clone();

                        async move {
                            let evt_sender = &evt_sender;

                            match event {
                                MetaNetEvt::Connected { remote_url, con } => {
                                    // TODO can this match be shared once everything is tested?
                                    match this.handle_connect(remote_url, con).await {
                                        Err(MetaNetTaskError::GhostActorClosed(_)) => {
                                            let _ = shutdown_send.send(()).await;
                                        }
                                        _ => {
                                            // Ignore anything else
                                        }
                                    }
                                }
                                MetaNetEvt::Disconnected { remote_url, con: _ } => {
                                    match this.handle_disconnect(remote_url).await {
                                        Err(MetaNetTaskError::GhostActorClosed(_)) => {
                                            let _ = shutdown_send.send(()).await;
                                        }
                                        _ => {
                                            // Ignore anything else
                                        }
                                    }
                                }
                                MetaNetEvt::Request {
                                    remote_url: _,
                                    con,
                                    data,
                                    respond,
                                } => {
                                    match nodespace_is_authorized(
                                        &host,
                                        con.peer_id(),
                                        data.maybe_space(),
                                        Timestamp::now(),
                                    )
                                        .await
                                    {
                                        MetaNetAuth::UnauthorizedIgnore => {}
                                        MetaNetAuth::UnauthorizedDisconnect => {
                                            con.close(
                                                UNAUTHORIZED_DISCONNECT_CODE,
                                                UNAUTHORIZED_DISCONNECT_REASON,
                                            )
                                                .await;
                                        }
                                        MetaNetAuth::Authorized => {
                                            match data {
                                                wire::Wire::Call(wire::Call {
                                                                     space,
                                                                     to_agent,
                                                                     data,
                                                                     ..
                                                                 }) => {
                                                    let res = match evt_sender
                                                        .call(space, to_agent, data.into())
                                                        .await
                                                    {
                                                        Err(err) => {
                                                            let reason = format!("{:?}", err);
                                                            let fail = wire::Wire::failure(reason);
                                                            respond(fail).await;
                                                            return;
                                                        }
                                                        Ok(r) => r,
                                                    };
                                                    let resp = wire::Wire::call_resp(res.into());
                                                    respond(resp).await;
                                                }
                                                wire::Wire::PeerGet(wire::PeerGet {
                                                                        space,
                                                                        agent,
                                                                    }) => {
                                                    let resp = match host
                                                        .get_agent_info_signed(
                                                            GetAgentInfoSignedEvt { space, agent },
                                                        )
                                                        .await
                                                    {
                                                        Ok(info) => wire::Wire::peer_get_resp(info),
                                                        Err(err) => wire::Wire::failure(format!(
                                                            "Error getting agent: {:?}",
                                                            err,
                                                        )),
                                                    };
                                                    respond(resp).await;
                                                }
                                                wire::Wire::PeerQuery(wire::PeerQuery {
                                                                          space,
                                                                          basis_loc,
                                                                      }) => {
                                                    // this *does* go over the network...
                                                    // so we don't want it to be too many
                                                    const LIMIT: u32 = 8;
                                                    let query = QueryAgentsEvt::new(space)
                                                        .near_basis(basis_loc)
                                                        .limit(LIMIT);
                                                    let resp = match evt_sender
                                                        .query_agents(query)
                                                        .await
                                                    {
                                                        Ok(list) => {
                                                            wire::Wire::peer_query_resp(list)
                                                        }
                                                        Err(err) => wire::Wire::failure(format!(
                                                            "Error querying agents: {:?}",
                                                            err,
                                                        )),
                                                    };
                                                    respond(resp).await;
                                                }
                                                data => unimplemented!("{:?}", data),
                                            }
                                        }
                                    }
                                }
                                MetaNetEvt::Notify {
                                    remote_url: url,
                                    con,
                                    data,
                                } => {
                                    match nodespace_is_authorized(
                                        &host,
                                        con.peer_id(),
                                        data.maybe_space(),
                                        Timestamp::now(),
                                    )
                                        .await
                                    {
                                        MetaNetAuth::UnauthorizedIgnore => {}
                                        MetaNetAuth::UnauthorizedDisconnect => {
                                            con.close(
                                                UNAUTHORIZED_DISCONNECT_CODE,
                                                UNAUTHORIZED_DISCONNECT_REASON,
                                            )
                                                .await;
                                        }
                                        MetaNetAuth::Authorized => {
                                            match data {
                                                wire::Wire::DelegateBroadcast(
                                                    wire::DelegateBroadcast {
                                                        space,
                                                        basis,
                                                        to_agent,
                                                        mod_idx,
                                                        mod_cnt,
                                                        data,
                                                    },
                                                ) => match data {
                                                    BroadcastData::Publish {
                                                        source,
                                                        op_hash_list,
                                                        context,
                                                    } => {
                                                        if let Err(err) = i_s
                                                            .incoming_publish(
                                                                space,
                                                                to_agent,
                                                                source,
                                                                op_hash_list,
                                                                context,
                                                                Some((basis, mod_idx, mod_cnt)),
                                                            )
                                                            .await
                                                        {
                                                            tracing::warn!(
                                                    ?err,
                                                    "failed to handle incoming delegate broadcast"
                                                );
                                                        }
                                                    }
                                                    data => {
                                                        // one might be tempted to notify here
                                                        // as in Broadcast below... but we
                                                        // notify all relevant agents inside
                                                        // the space incoming_delegate_broadcast
                                                        // handler.
                                                        if let Err(err) = i_s
                                                            .incoming_delegate_broadcast(
                                                                space, basis, to_agent, mod_idx,
                                                                mod_cnt, data,
                                                            )
                                                            .await
                                                        {
                                                            tracing::warn!(
                                                    ?err,
                                                    "failed to handle incoming delegate broadcast"
                                                );
                                                        }
                                                    }
                                                },
                                                wire::Wire::Broadcast(wire::Broadcast {
                                                                          space,
                                                                          to_agent,
                                                                          data,
                                                                          ..
                                                                      }) => match data {
                                                    BroadcastData::User(data) => {
                                                        // TODO: Should we check if the basis is
                                                        // held before calling notify?
                                                        if let Err(err) = evt_sender
                                                            .notify(space, to_agent, data)
                                                            .await
                                                        {
                                                            tracing::warn!(
                                                        ?err,
                                                        "error processing incoming broadcast"
                                                    );
                                                        }
                                                    }
                                                    BroadcastData::AgentInfo(agent_info) => {
                                                        // TODO: Should we check if the basis is
                                                        // held before calling put_agent_info_signed?
                                                        if let Err(err) = evt_sender
                                                            .put_agent_info_signed(
                                                                PutAgentInfoSignedEvt {
                                                                    space,
                                                                    peer_data: vec![agent_info],
                                                                },
                                                            )
                                                            .await
                                                        {
                                                            tracing::warn!(
                                                    ?err,
                                                    "error processing incoming agent info broadcast"
                                                );
                                                        }
                                                    }
                                                    BroadcastData::Publish {
                                                        source,
                                                        op_hash_list,
                                                        context,
                                                    } => {
                                                        if let Err(err) = i_s
                                                            .incoming_publish(
                                                                space,
                                                                to_agent,
                                                                source,
                                                                op_hash_list,
                                                                context,
                                                                None,
                                                            )
                                                            .await
                                                        {
                                                            tracing::warn!(
                                                        ?err,
                                                        "failed to handle incoming broadcast"
                                                    );
                                                        }
                                                    }
                                                },
                                                wire::Wire::Gossip(wire::Gossip {
                                                                       space,
                                                                       data,
                                                                       module,
                                                                   }) => {
                                                    let data: Vec<u8> = data.into();
                                                    let data: Box<[u8]> = data.into_boxed_slice();
                                                    if let Err(e) = i_s
                                                        .incoming_gossip(
                                                            space, con, url, data, module,
                                                        )
                                                        .await
                                                    {
                                                        tracing::warn!(
                                                    "failed to handle incoming gossip: {:?}",
                                                    e
                                                );
                                                    }
                                                }
                                                wire::Wire::FetchOp(wire::FetchOp {
                                                                        fetch_list,
                                                                    }) => {
                                                    for (space, key_list) in fetch_list {
                                                        let mut hashes = Vec::new();
                                                        let topo = match host
                                                            .get_topology(space.clone())
                                                            .await
                                                        {
                                                            Err(_) => continue,
                                                            Ok(topo) => topo,
                                                        };
                                                        let mut regions = Vec::new();

                                                        for key in key_list {
                                                            match key {
                                                                FetchKey::Region(region_coords) => {
                                                                    regions.push((
                                                                        region_coords,
                                                                        region_coords
                                                                            .to_bounds(&topo),
                                                                    ));
                                                                }
                                                                FetchKey::Op(op_hash) => {
                                                                    hashes.push(op_hash);
                                                                }
                                                            }
                                                        }

                                                        if !hashes.is_empty() {
                                                            if let Ok(list) = evt_sender
                                                                .fetch_op_data(FetchOpDataEvt {
                                                                    space: space.clone(),
                                                                    query: FetchOpDataEvtQuery::Hashes {
                                                                        op_hash_list: hashes,
                                                                        include_limbo: true,
                                                                    },
                                                                })
                                                                .await
                                                            {
                                                                for (_hash, op) in list {
                                                                    this.fetch_response_queue.enqueue_op(
                                                                        space.clone(),
                                                                        (con.clone(), url.clone(), None),
                                                                        op,
                                                                    );
                                                                }
                                                            }
                                                        }

                                                        for (coord, bound) in regions {
                                                            if let Ok(list) = evt_sender
                                                                .fetch_op_data(FetchOpDataEvt {
                                                                    space: space.clone(),
                                                                    query: FetchOpDataEvtQuery::Regions(
                                                                        vec![bound],
                                                                    ),
                                                                })
                                                                .await
                                                            {
                                                                let last_idx = list.len() - 1;
                                                                for (idx, (_hash, op)) in
                                                                list.into_iter().enumerate()
                                                                {
                                                                    this.fetch_response_queue.enqueue_op(
                                                                        space.clone(),
                                                                        (
                                                                            con.clone(),
                                                                            url.clone(),
                                                                            Some((coord, idx == last_idx)),
                                                                        ),
                                                                        op,
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                wire::Wire::PushOpData(wire::PushOpData {
                                                                           op_data_list,
                                                                       }) => {
                                                    for (space, op_list) in op_data_list {
                                                        for op in op_list {
                                                            // hash the op
                                                            let op_hash = match host
                                                                .op_hash(op.op_data.clone())
                                                                .await
                                                            {
                                                                Ok(op_hash) => op_hash,
                                                                Err(_) => continue,
                                                            };

                                                            // trigger any delegation
                                                            // that is pending on
                                                            // having this data
                                                            let _ = i_s
                                                                .resolve_publish_pending_delegates(
                                                                    space.clone(),
                                                                    op_hash.clone(),
                                                                )
                                                                .await;

                                                            // MAYBE: do something with the
                                                            //        is_last bool?
                                                            //        Right now we don't
                                                            //        really care, because
                                                            //        if it's a region
                                                            //        we know it's gossip
                                                            //        so it's okay if
                                                            //        the context is
                                                            //        `None`.
                                                            let key =
                                                                if let Some((region, _is_last)) =
                                                                    op.region
                                                                {
                                                                    FetchKey::Region(region)
                                                                } else {
                                                                    FetchKey::Op(op_hash.clone())
                                                                };
                                                            let fetch_context = this.fetch_pool
                                                                .remove(&key)
                                                                .and_then(|i| i.context);

                                                            // forward the received op
                                                            let _ = evt_sender
                                                                .receive_ops(
                                                                    space.clone(),
                                                                    vec![op.op_data],
                                                                    fetch_context,
                                                                )
                                                                .await;
                                                        }
                                                    }
                                                }
                                                wire::Wire::MetricExchange(
                                                    wire::MetricExchange { space, msgs },
                                                ) => {
                                                    let _ = i_s
                                                        .incoming_metric_exchange(space, msgs)
                                                        .await;
                                                }
                                                wire::Wire::PeerUnsolicited(
                                                    wire::PeerUnsolicited { peer_list },
                                                ) => {
                                                    for peer in peer_list {
                                                        if let Err(err) = evt_sender
                                                            .put_agent_info_signed(
                                                                PutAgentInfoSignedEvt {
                                                                    space: peer.space.clone(),
                                                                    peer_data: vec![peer.clone()],
                                                                },
                                                            ).await {
                                                            tracing::warn!(?err, "error processing incoming agent info unsolicited");
                                                        }
                                                    }
                                                }
                                                wire::Wire::Failure(_)
                                                | wire::Wire::Call(_)
                                                | wire::Wire::CallResp(_)
                                                | wire::Wire::PeerGet(_)
                                                | wire::Wire::PeerGetResp(_)
                                                | wire::Wire::PeerQuery(_)
                                                | wire::Wire::PeerQueryResp(_) => {
                                                    tracing::warn!(
                                                        "received non-notify data in a notify"
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });

                tokio::select! {
                    _ = ep_evt_run => {
                        // This will happen if all senders close
                    }
                    _ = shutdown_recv.next() => {
                        // Got a shutdown signal
                    }
                }

                tracing::error!(
                    "KitsuneP2p: networking poll shutdown. Networking will no longer work!
                You can ignore this is if it happened during node shutdown.
                Otherwise please restart your node and report this error."
                );
                is_finished.fetch_or(true, Ordering::SeqCst)
            }
        });
    }

    async fn handle_connect(&self, remote_url: String, con: MetaNetCon) -> MetaNetTaskResult<()> {
        match self.i_s.new_con(remote_url, con.clone()).await {
            Err(KitsuneP2pError::GhostError(e)) => match e {
                ghost_actor::GhostError::Disconnected => Err(e.into()),
                _ => Err(MetaNetTaskError::Ignored),
            },
            Err(_) => Err(MetaNetTaskError::Ignored),
            Ok(_) => Ok(()),
        }
    }

    async fn handle_disconnect(&self, remote_url: String) -> MetaNetTaskResult<()> {
        match self.i_s.del_con(remote_url).await {
            Err(KitsuneP2pError::GhostError(e)) => match e {
                ghost_actor::GhostError::Disconnected => Err(e.into()),
                _ => Err(MetaNetTaskError::Ignored),
            },
            Err(_) => Err(MetaNetTaskError::Ignored),
            Ok(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::spawn::actor::fetch::FetchResponseConfig;
    use crate::spawn::actor::meta_net_task::MetaNetTask;
    use crate::spawn::actor::test_util::InternalStub;
    use crate::spawn::actor::Internal;
    use crate::spawn::meta_net::{MetaNetCon, MetaNetEvt};
    use crate::HostStub;
    use futures::channel::mpsc::{channel, Sender};
    use futures::SinkExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlSender, GhostSender};
    use kitsune_p2p_fetch::{FetchPool, FetchResponseQueue};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_connect() {
        let (mut ep_evt_send, internal_stub, _, _) = setup().await;

        assert_eq!(0, internal_stub.connections.read().len());

        ep_evt_send
            .send(MetaNetEvt::Connected {
                remote_url: "".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(100), async {
            while internal_stub.connections.read().is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("Timed out waiting for connection to be added");

        assert_eq!(1, internal_stub.connections.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_connect_stops_task_if_internal_sender_closes() {
        let (mut ep_evt_send, internal_stub, internal_sender, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Connected {
                remote_url: "".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(1000), async {
            while !meta_net_task_finished.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("Timed out waiting for task to shut down");

        assert!(meta_net_task_finished.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_disconnect() {
        let (mut ep_evt_send, internal_stub, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Connected {
                remote_url: "x".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Disconnected {
                remote_url: "x".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(100), async {
            while !internal_stub.connections.read().is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("Timed out waiting for connection to be removed");

        assert_eq!(0, internal_stub.connections.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_disconnect_stops_task_if_internal_sender_closes() {
        let (mut ep_evt_send, internal_stub, internal_sender, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Disconnected {
                remote_url: "".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(1000), async {
            while !meta_net_task_finished.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("Timed out waiting for task to shut down");

        assert!(meta_net_task_finished.load(Ordering::Acquire));
    }

    async fn setup() -> (
        Sender<MetaNetEvt>,
        InternalStub,
        GhostSender<Internal>,
        Arc<AtomicBool>,
    ) {
        let task = InternalStub::new();

        let builder = GhostActorBuilder::new();

        let internal_sender = builder
            .channel_factory()
            .create_channel::<Internal>()
            .await
            .unwrap();

        let (host_sender, host_receiver) = channel(10);

        tokio::spawn(builder.spawn(task.clone()));

        let host_stub = HostStub::new();

        let fetch_pool = FetchPool::new_bitwise_or();

        let fetch_response_queue =
            FetchResponseQueue::new(FetchResponseConfig::new(Default::default()));

        let (ep_evt_send, ep_evt_rcv) = channel(10);

        let meta_net_task = MetaNetTask::new(
            host_sender,
            host_stub,
            Default::default(),
            fetch_pool,
            fetch_response_queue,
            ep_evt_rcv,
            internal_sender.clone(),
        );
        let meta_net_task_finished = meta_net_task.is_finished.clone();

        meta_net_task.spawn();

        (ep_evt_send, task, internal_sender, meta_net_task_finished)
    }

    fn mk_test_con() -> MetaNetCon {
        MetaNetCon::Test {
            state: Default::default(),
        }
    }
}
