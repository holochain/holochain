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
    nodespace_is_authorized, MetaNetAuth, MetaNetCon, MetaNetEvt, MetaNetEvtRecv, Respond,
};
use crate::wire::WireData;
use crate::{wire, HostApi, KitsuneAgent, KitsuneP2pConfig, KitsuneP2pError, KitsuneSpace};
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
                                                    this.handle_call_request(space, to_agent, data, respond).await;
                                                }
                                                wire::Wire::PeerGet(wire::PeerGet {
                                                                        space,
                                                                        agent,
                                                                    }) => {
                                                    this.handle_peer_get_request(space, agent, respond).await;
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

    async fn handle_call_request(
        &self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        data: WireData,
        respond: Respond,
    ) {
        let res = match self.evt_sender.call(space, to_agent, data.into()).await {
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

    async fn handle_peer_get_request(
        &self,
        space: Arc<KitsuneSpace>,
        agent: Arc<KitsuneAgent>,
        respond: Respond,
    ) {
        let resp = match self
            .host
            .get_agent_info_signed(GetAgentInfoSignedEvt { space, agent })
            .await
        {
            Ok(info) => wire::Wire::peer_get_resp(info),
            Err(err) => wire::Wire::failure(format!("Error getting agent: {:?}", err,)),
        };
        respond(resp).await;
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::BroadcastData;
    use crate::dht_arc::DhtLocation;
    use crate::spawn::actor::fetch::FetchResponseConfig;
    use crate::spawn::actor::meta_net_task::MetaNetTask;
    use crate::spawn::actor::test_util::HostStub as HostReceiverStub;
    use crate::spawn::actor::test_util::InternalStub;
    use crate::spawn::actor::Internal;
    use crate::spawn::meta_net::{MetaNetCon, MetaNetConTest, MetaNetEvt};
    use crate::types::wire;
    use crate::wire::{Wire, WireData};
    use crate::{HostStub, KitsuneAgent, KitsuneHost};
    use futures::channel::mpsc::{channel, Sender};
    use futures::FutureExt;
    use futures::SinkExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlSender, GhostSender};
    use kitsune_p2p::KitsuneBinType;
    use kitsune_p2p_block::{Block, BlockTarget, NodeBlockReason, NodeId};
    use kitsune_p2p_fetch::test_utils::test_space;
    use kitsune_p2p_fetch::{FetchPool, FetchResponseQueue};
    use kitsune_p2p_timestamp::{InclusiveTimestampInterval, Timestamp};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_connect() {
        let (mut ep_evt_send, internal_stub, _, _, _, _) = setup().await;

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
        let (mut ep_evt_send, internal_stub, internal_sender, _, _, meta_net_task_finished) =
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
        let (mut ep_evt_send, internal_stub, _, _, _, _) = setup().await;

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
        let (mut ep_evt_send, internal_stub, internal_sender, _, _, meta_net_task_finished) =
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

    #[tokio::test(flavor = "multi_thread")]
    async fn make_request_while_blocked() {
        let (mut ep_evt_send, internal_stub, internal_sender, _, host_stub, meta_net_task_finished) =
            setup().await;

        host_stub
            .block(Block::new(
                BlockTarget::Node(test_node_id(1), NodeBlockReason::DOS),
                InclusiveTimestampInterval::try_new(
                    Timestamp::now(),
                    Timestamp::now()
                        .checked_add(&Duration::from_secs(10))
                        .unwrap(),
                )
                .unwrap(),
            ))
            .await
            .unwrap();

        let con = mk_test_con_with_id(1);
        let con_state = get_con_state(&con);

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: con,
                data: wire::Wire::Call(wire::Call {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: WireData(vec![]),
                }),
                respond: Box::new(|_| async move { () }.boxed().into()),
            })
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(1000), async {
            while !con_state.read().closed {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("Timed out waiting for the connection to be closed");

        assert!(con_state.read().closed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_call_request() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            _,
            meta_net_task_finished,
        ) = setup().await;

        let (send_res, read_res) = futures::channel::oneshot::channel();

        let request_data = vec![2, 7];
        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::Call(wire::Call {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: WireData(request_data.clone()),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let call_response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let response_data = match call_response {
            Wire::CallResp(res) => res.data.to_vec(),
            _ => panic!("Unexpected response"),
        };

        // Because the stub does an echo response
        assert_eq!(request_data, response_data);
    }

    // TODO This is actually a fatal error so the task should stop and then I need another mechanism to cause an error
    //      to test the behaviour this test is currently checking.
    #[tokio::test(flavor = "multi_thread")]
    async fn make_call_request_after_host_closed() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            _,
            meta_net_task_finished,
        ) = setup().await;

        host_receiver_stub.abort();

        let (send_res, read_res) = futures::channel::oneshot::channel();

        let request_data = vec![2, 7];
        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::Call(wire::Call {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: WireData(request_data.clone()),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let call_response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let reason = match call_response {
            Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!("GhostError(Disconnected)".to_string(), reason);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_peer_get_request() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            _,
            meta_net_task_finished,
        ) = setup().await;

        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerGet(wire::PeerGet {
                    space: test_space(1),
                    agent: test_agent(1),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let call_response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let agent_info_signed = match call_response {
            Wire::PeerGetResp(res) => res.agent_info_signed,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(test_agent(1), agent_info_signed.unwrap().agent);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_peer_get_request_error() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            host_stub,
            meta_net_task_finished,
        ) = setup().await;

        // Set up the error response so that when we make a request we get an error
        host_stub.fail_next_request();

        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerGet(wire::PeerGet {
                    space: test_space(1),
                    agent: test_agent(1),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let call_response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let reason = match call_response {
            Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!("Error getting agent: \"error for unimplemented KitsuneHost test behavior: method get_agent_info_signed of HostStub\"".to_string(), reason);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_peer_query_request() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            host_stub,
            meta_net_task_finished,
        ) = setup().await;

        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerQuery(wire::PeerQuery {
                    space: test_space(1),
                    basis_loc: DhtLocation::new(1),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let peer_list = match response {
            Wire::PeerQueryResp(r) => r.peer_list,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(8, peer_list.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_peer_query_request_error() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            host_stub,
            meta_net_task_finished,
        ) = setup().await;

        // Set up the error response so that when we make a request we get an error
        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerQuery(wire::PeerQuery {
                    space: test_space(1),
                    basis_loc: DhtLocation::new(1),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let reason = match response {
            Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(
            "Error querying agents: Other(\"a test error\")".to_string(),
            reason
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "This crashes the process because it's hitting an `unimplemented` condition"]
    async fn ignores_unexpected_request_payload() {
        let (
            mut ep_evt_send,
            internal_stub,
            internal_sender,
            host_receiver_stub,
            host_stub,
            meta_net_task_finished,
        ) = setup().await;

        // Send a request but don't listen for a response
        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(1),
                    data: BroadcastData::User(test_agent(2).to_vec()),
                }),
                respond: Box::new(|r| async move { () }.boxed().into()),
            })
            .await
            .unwrap();

        // Now check that we can still use the task to send/receive messages.
        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerQuery(wire::PeerQuery {
                    space: test_space(1),
                    basis_loc: DhtLocation::new(1),
                }),
                respond: Box::new(|r| {
                    async move {
                        send_res.send(r).unwrap();
                        ()
                    }
                    .boxed()
                    .into()
                }),
            })
            .await
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap();

        let peer_list = match response {
            Wire::PeerQueryResp(r) => r.peer_list,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(8, peer_list.len());
    }

    async fn setup() -> (
        Sender<MetaNetEvt>,
        InternalStub,
        GhostSender<Internal>,
        HostReceiverStub,
        Arc<HostStub>,
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
        let host_receiver_stub = HostReceiverStub::start(host_receiver);

        tokio::spawn(builder.spawn(task.clone()));

        let host_stub = HostStub::new();

        let fetch_pool = FetchPool::new_bitwise_or();

        let fetch_response_queue =
            FetchResponseQueue::new(FetchResponseConfig::new(Default::default()));

        let (ep_evt_send, ep_evt_rcv) = channel(10);

        let meta_net_task = MetaNetTask::new(
            host_sender,
            host_stub.clone(),
            Default::default(),
            fetch_pool,
            fetch_response_queue,
            ep_evt_rcv,
            internal_sender.clone(),
        );
        let meta_net_task_finished = meta_net_task.is_finished.clone();

        meta_net_task.spawn();

        (
            ep_evt_send,
            task,
            internal_sender,
            host_receiver_stub,
            host_stub,
            meta_net_task_finished,
        )
    }

    fn mk_test_con() -> MetaNetCon {
        MetaNetCon::Test {
            state: Default::default(),
        }
    }

    fn mk_test_con_with_id(id: u8) -> MetaNetCon {
        MetaNetCon::Test {
            state: Arc::new(parking_lot::RwLock::new(MetaNetConTest::new_with_id(id))),
        }
    }

    fn test_node_id(i: u8) -> NodeId {
        Arc::new(vec![i; 32].try_into().unwrap())
    }

    fn test_agent(i: u8) -> Arc<KitsuneAgent> {
        Arc::new(KitsuneAgent::new(vec![i; 32]))
    }

    fn get_con_state(con: &MetaNetCon) -> Arc<parking_lot::RwLock<MetaNetConTest>> {
        match con {
            MetaNetCon::Test { state } => state.clone(),
            _ => panic!("Not a test con"),
        }
    }
}
