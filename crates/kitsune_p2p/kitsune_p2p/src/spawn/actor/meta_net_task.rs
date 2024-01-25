use crate::actor::BroadcastData;
use crate::event::{
    FetchOpDataEvt, FetchOpDataEvtQuery, GetAgentInfoSignedEvt, KitsuneP2pEventSender,
    PutAgentInfoSignedEvt, QueryAgentsEvt,
};
use crate::spawn::actor::fetch::FetchResponseConfig;
use crate::spawn::actor::{
    Internal, InternalSender, UNAUTHORIZED_DISCONNECT_CODE, UNAUTHORIZED_DISCONNECT_REASON,
};
use crate::spawn::meta_net::{
    nodespace_is_authorized, MetaNetAuth, MetaNetCon, MetaNetEvt, MetaNetEvtRecv, Respond,
};
use crate::wire::WireData;
use crate::{wire, HostApiLegacy, KitsuneAgent, KitsuneP2pError, KitsuneSpace};
use futures::StreamExt;
use ghost_actor::{GhostError, GhostSender};
use kitsune_p2p_fetch::{FetchKey, FetchPool, FetchResponseQueue};
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::config::KitsuneP2pConfig;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::Instrument;

pub struct MetaNetTask {
    host: HostApiLegacy,
    config: KitsuneP2pConfig,
    fetch_pool: FetchPool,
    fetch_response_queue: FetchResponseQueue<FetchResponseConfig>,
    ep_evt: Option<MetaNetEvtRecv>,
    i_s: GhostSender<Internal>,
    is_finished: Arc<AtomicBool>,
}

#[derive(thiserror::Error, Debug)]
enum MetaNetTaskError {
    #[error("A required channel has closed")]
    RequiredChannelClosed,

    #[error("Ignored error: {0}")]
    Ignored(Box<dyn Error>),
}

impl From<KitsuneP2pError> for MetaNetTaskError {
    fn from(err: KitsuneP2pError) -> Self {
        match err {
            KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                MetaNetTaskError::RequiredChannelClosed
            }
            e => MetaNetTaskError::Ignored(Box::new(e)),
        }
    }
}

type MetaNetTaskResult<T> = Result<T, MetaNetTaskError>;

impl MetaNetTask {
    pub fn new(
        host: HostApiLegacy,
        config: KitsuneP2pConfig,
        fetch_pool: FetchPool,
        fetch_response_queue: FetchResponseQueue<FetchResponseConfig>,
        ep_evt: MetaNetEvtRecv,
        i_s: GhostSender<Internal>,
    ) -> Self {
        Self {
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
        let shutdown_notify = Arc::new(tokio::sync::Notify::new());
        let shutdown_notify_send = shutdown_notify.clone();

        let is_finished = self.is_finished.clone();

        tokio::task::spawn({
            let tuning_params = self.config.tuning_params.clone();
            let span =
                tracing::error_span!("MetaNetTask::spawn", scope = self.config.tracing_scope);
            let span_outer = span.clone();
            async move {
                let ep_evt = self
                    .ep_evt
                    .take()
                    .expect("There should always be an ep_evt");

                let this = Arc::new(self);
                let span = span.clone();

                let ep_evt_run = ep_evt.for_each_concurrent(
                    tuning_params.concurrent_limit_per_thread,
                    move |event| {
                        let this = this.clone();
                        let shutdown_notify = shutdown_notify_send.clone();
                        let span = span.clone();
                        async move {
                            if let Err(MetaNetTaskError::RequiredChannelClosed) = match event {
                                MetaNetEvt::Connected { remote_url, con } => {
                                    this.handle_connect(remote_url, con).await
                                }
                                MetaNetEvt::Disconnected { remote_url, con: _ } => {
                                    this.handle_disconnect(remote_url).await
                                }
                                MetaNetEvt::Request {
                                    remote_url: _,
                                    con,
                                    data,
                                    respond,
                                } => this.handle_request(con, data, respond).await,
                                MetaNetEvt::Notify {
                                    remote_url: url,
                                    con,
                                    data,
                                } => this.handle_notify(url, con, data).await,
                            } {
                                shutdown_notify.notify_one();
                            }
                        }
                        .instrument(span)
                    },
                );

                tokio::select! {
                    _ = ep_evt_run => {
                        // This will happen if all senders close
                    }
                    _ = shutdown_notify.notified() => {
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
            .instrument(span_outer)
        });
    }

    async fn handle_connect(&self, remote_url: String, con: MetaNetCon) -> MetaNetTaskResult<()> {
        match self.i_s.new_con(remote_url, con.clone()).await {
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }

    async fn handle_disconnect(&self, remote_url: String) -> MetaNetTaskResult<()> {
        match self.i_s.del_con(remote_url).await {
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }

    async fn handle_request(
        &self,
        con: MetaNetCon,
        data: wire::Wire,
        respond: Respond,
    ) -> MetaNetTaskResult<()> {
        match nodespace_is_authorized(
            &self.host,
            con.peer_id(),
            data.maybe_space(),
            Timestamp::now(),
        )
        .await
        {
            MetaNetAuth::UnauthorizedIgnore => {}
            MetaNetAuth::UnauthorizedDisconnect => {
                con.close(UNAUTHORIZED_DISCONNECT_CODE, UNAUTHORIZED_DISCONNECT_REASON)
                    .await;
            }
            MetaNetAuth::Authorized => {
                self.handle_request_authorized(data, respond).await?;
            }
        }

        Ok(())
    }

    async fn handle_request_authorized(
        &self,
        data: wire::Wire,
        respond: Respond,
    ) -> MetaNetTaskResult<()> {
        match data {
            wire::Wire::Call(wire::Call {
                space,
                to_agent,
                data,
                ..
            }) => {
                self.handle_call_request(space, to_agent, data, respond)
                    .await?;
            }
            wire::Wire::PeerGet(wire::PeerGet { space, agent }) => {
                self.handle_peer_get_request(space, agent, respond).await;
            }
            wire::Wire::PeerQuery(wire::PeerQuery { space, basis_loc }) => {
                // this *does* go over the network...
                // so we don't want it to be too many
                const LIMIT: u32 = 8;
                let query = QueryAgentsEvt::new(space)
                    .near_basis(basis_loc)
                    .limit(LIMIT);
                let resp = match self.host.legacy.query_agents(query).await {
                    Ok(list) => wire::Wire::peer_query_resp(list),
                    Err(err) => wire::Wire::failure(format!("Error querying agents: {:?}", err,)),
                };
                respond(resp).await;
            }
            _ => {
                tracing::warn!("received non-request data in a request");
            }
        }

        Ok(())
    }

    async fn handle_call_request(
        &self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        data: WireData,
        respond: Respond,
    ) -> MetaNetTaskResult<()> {
        let res = match self.host.legacy.call(space, to_agent, data.into()).await {
            Err(err) => {
                let reason = format!("{:?}", err);
                let fail = wire::Wire::failure(reason);
                respond(fail).await;

                return Err(err.into());
            }
            Ok(r) => r,
        };
        let resp = wire::Wire::call_resp(res.into());
        respond(resp).await;

        Ok(())
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

    async fn handle_notify(
        &self,
        url: String,
        con: MetaNetCon,
        data: wire::Wire,
    ) -> MetaNetTaskResult<()> {
        match nodespace_is_authorized(
            &self.host,
            con.peer_id(),
            data.maybe_space(),
            Timestamp::now(),
        )
        .await
        {
            MetaNetAuth::UnauthorizedIgnore => {}
            MetaNetAuth::UnauthorizedDisconnect => {
                con.close(UNAUTHORIZED_DISCONNECT_CODE, UNAUTHORIZED_DISCONNECT_REASON)
                    .await;
            }
            MetaNetAuth::Authorized => {
                self.handle_notify_authorized(url, con, data).await?;
            }
        }

        Ok(())
    }

    async fn handle_notify_authorized(
        &self,
        url: String,
        con: MetaNetCon,
        data: wire::Wire,
    ) -> MetaNetTaskResult<()> {
        match data {
            wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                space,
                basis,
                to_agent,
                mod_idx,
                mod_cnt,
                data,
            }) => match data {
                BroadcastData::Publish {
                    source,
                    op_hash_list,
                    context,
                } => {
                    if let Err(err) = self
                        .i_s
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
                        tracing::warn!(?err, "failed to handle incoming delegate broadcast");
                        Err(err.into())
                    } else {
                        Ok(())
                    }
                }
                data => {
                    // one might be tempted to notify here
                    // as in Broadcast below... but we
                    // notify all relevant agents inside
                    // the space incoming_delegate_broadcast
                    // handler.
                    if let Err(err) = self
                        .i_s
                        .incoming_delegate_broadcast(space, basis, to_agent, mod_idx, mod_cnt, data)
                        .await
                    {
                        tracing::warn!(?err, "failed to handle incoming delegate broadcast");
                        Err(err.into())
                    } else {
                        Ok(())
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
                    // TODO: Should we check if the basis is held before calling notify?
                    if let Err(err) = self.host.legacy.notify(space, to_agent, data).await {
                        tracing::warn!(?err, "error processing incoming broadcast");
                        Err(err.into())
                    } else {
                        Ok(())
                    }
                }
                BroadcastData::AgentInfo(agent_info) => {
                    // TODO: Should we check if the basis is
                    //       held before calling put_agent_info_signed?
                    if let Err(err) = self
                        .host
                        .legacy
                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                            space,
                            peer_data: vec![agent_info],
                        })
                        .await
                    {
                        tracing::warn!(?err, "error processing incoming agent info broadcast");
                        Err(err.into())
                    } else {
                        Ok(())
                    }
                }
                BroadcastData::Publish {
                    source,
                    op_hash_list,
                    context,
                } => {
                    if let Err(err) = self
                        .i_s
                        .incoming_publish(space, to_agent, source, op_hash_list, context, None)
                        .await
                    {
                        tracing::warn!(?err, "failed to handle incoming broadcast");
                        Err(err.into())
                    } else {
                        Ok(())
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
                if let Err(err) = self
                    .i_s
                    .incoming_gossip(space, con, url, data, module)
                    .await
                {
                    tracing::warn!(?err, "failed to handle incoming gossip");
                    Err(err.into())
                } else {
                    Ok(())
                }
            }
            wire::Wire::FetchOp(wire::FetchOp { fetch_list }) => {
                for (space, key_list) in fetch_list {
                    let mut hashes = Vec::new();
                    for key in key_list {
                        let FetchKey::Op(op_hash) = key;
                        hashes.push(op_hash);
                    }

                    if !hashes.is_empty() {
                        match self
                            .host
                            .legacy
                            .fetch_op_data(FetchOpDataEvt {
                                space: space.clone(),
                                query: FetchOpDataEvtQuery::Hashes {
                                    op_hash_list: hashes,
                                    include_limbo: true,
                                },
                            })
                            .await
                        {
                            Ok(list) => {
                                for (_hash, op) in list {
                                    self.fetch_response_queue.enqueue_op(
                                        space.clone(),
                                        (con.clone(), url.clone(), None),
                                        op,
                                    );
                                }
                            }
                            Err(KitsuneP2pError::GhostError(GhostError::Disconnected)) => {
                                return Err(MetaNetTaskError::RequiredChannelClosed)
                            }
                            _ => {
                                // Ignore other errors
                            }
                        }
                    }
                }

                Ok(())
            }
            wire::Wire::PushOpData(wire::PushOpData { op_data_list }) => {
                for (space, op_list) in op_data_list {
                    for op in op_list {
                        // hash the op
                        let op_hash = match self.host.op_hash(op.op_data.clone()).await {
                            Ok(op_hash) => op_hash,
                            Err(err) => {
                                tracing::warn!(
                                    ?err,
                                    "Dropping incoming op because the host failed to hash it {:?}",
                                    op
                                );
                                continue;
                            }
                        };

                        let key = FetchKey::Op(op_hash.clone());
                        let fetch_context = match self.fetch_pool.check_item(&key) {
                            (true, maybe_fetch_context) => maybe_fetch_context,
                            (false, _) => {
                                tracing::warn!(
                                    "Dropping incoming op because the fetch pool did not contain it, this may indicate a hashing mismatch or unsolicited pushes {:?}",
                                    op
                                );
                                continue;
                            }
                        };

                        // forward the received op
                        if let Err(err) = self
                            .host
                            .legacy
                            .receive_ops(space.clone(), vec![op.op_data], fetch_context)
                            .await
                        {
                            match err {
                                KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                                    return Err(MetaNetTaskError::RequiredChannelClosed)
                                }
                                err => {
                                    tracing::error!(?err, "Failed to receive op");
                                }
                            }

                            // In the case of an error we don't want to attempt to `resolve_publish_pending_delegates`
                            continue;
                        }

                        // Now that the host is holding the op, remove it from the fetch pool. Any sooner and we might queue the op for fetching again.
                        // We don't need to wait for validation to complete, at least with respect to gossip, because we don't ask for unvalidated
                        // ops during gossip. (See crates/holochain/src/conductor/kitsune_host_impl/query_region_set.rs)
                        self.fetch_pool.remove(&key);

                        // trigger any delegation that is pending on having this data
                        if let Err(err) = self
                            .i_s
                            .resolve_publish_pending_delegates(space.clone(), op_hash.clone())
                            .await
                        {
                            match err {
                                KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                                    return Err(MetaNetTaskError::RequiredChannelClosed);
                                }
                                err => {
                                    tracing::error!(
                                        ?err,
                                        "Failed to send notification to resolve pending delegates"
                                    );
                                }
                            }
                        }
                    }
                }

                Ok(())
            }
            wire::Wire::MetricExchange(wire::MetricExchange { space, msgs }) => {
                if let Err(err) = self.i_s.incoming_metric_exchange(space, msgs).await {
                    tracing::error!(?err, "Metric exchange failed to send");
                    Err(err.into())
                } else {
                    Ok(())
                }
            }
            wire::Wire::PeerUnsolicited(wire::PeerUnsolicited { peer_list }) => {
                for peer in peer_list {
                    if let Err(err) = self
                        .host
                        .legacy
                        .put_agent_info_signed(PutAgentInfoSignedEvt {
                            space: peer.space.clone(),
                            peer_data: vec![peer.clone()],
                        })
                        .await
                    {
                        tracing::warn!(?err, "error processing incoming agent info unsolicited");

                        match err {
                            KitsuneP2pError::GhostError(GhostError::Disconnected) => {
                                return Err(MetaNetTaskError::RequiredChannelClosed)
                            }
                            e => {
                                tracing::error!("Failed to put agent info: {:?}", e);
                            }
                        };
                    }
                }

                Ok(())
            }
            wire::Wire::Failure(_)
            | wire::Wire::Call(_)
            | wire::Wire::CallResp(_)
            | wire::Wire::PeerGet(_)
            | wire::Wire::PeerGetResp(_)
            | wire::Wire::PeerQuery(_)
            | wire::Wire::PeerQueryResp(_) => {
                tracing::warn!("received non-notify data in a notify");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::BroadcastData;
    use crate::dht_arc::DhtLocation;
    use crate::spawn::actor::fetch::FetchResponseConfig;
    use crate::spawn::actor::meta_net_task::MetaNetTask;
    use crate::spawn::actor::test_util::InternalStub;
    use crate::spawn::actor::test_util::LegacyHostStub as HostReceiverStub;
    use crate::spawn::actor::Internal;
    use crate::spawn::meta_net::{MetaNetCon, MetaNetConTest, MetaNetEvt};
    use crate::test_util::data::mk_agent_info;
    use crate::types::wire;
    use crate::wire::PushOpItem;
    use crate::{
        GossipModuleType, HostStub, KitsuneAgent, KitsuneBasis, KitsuneHost, KitsuneOpData,
    };
    use futures::channel::mpsc::{channel, Sender};
    use futures::FutureExt;
    use futures::SinkExt;
    use ghost_actor::actor_builder::GhostActorBuilder;
    use ghost_actor::{GhostControlSender, GhostSender};
    use kitsune_p2p::KitsuneBinType;
    use kitsune_p2p_block::{Block, BlockTarget, NodeBlockReason, NodeId};
    use kitsune_p2p_fetch::test_utils::{test_key_op, test_req_op, test_source, test_space};
    use kitsune_p2p_fetch::{FetchPool, FetchResponseQueue};
    use kitsune_p2p_timestamp::{InclusiveTimestampInterval, Timestamp};
    use kitsune_p2p_types::bin_types::NodeCert;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_connect() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        assert_eq!(0, internal_stub.connections.read().len());

        ep_evt_send
            .send(MetaNetEvt::Connected {
                remote_url: "".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        wait_for_condition(|| !internal_stub.connections.read().is_empty())
            .await
            .expect("Timed out waiting for connection to be added");

        assert_eq!(1, internal_stub.connections.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_connect_stops_task_if_internal_sender_closes() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, meta_net_task_finished) =
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

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_disconnect() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Connected {
                remote_url: "x".to_string(),
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
        .expect("Timed out waiting for connection to be removed");

        ep_evt_send
            .send(MetaNetEvt::Disconnected {
                remote_url: "x".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        wait_for_condition(|| internal_stub.connections.read().is_empty())
            .await
            .expect("Timed out waiting for connection to be removed");

        assert_eq!(0, internal_stub.connections.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_disconnect_stops_task_if_internal_sender_closes() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        wait_for_condition(|| !meta_net_task_finished.load(Ordering::Acquire))
            .await
            .expect("Timed out waiting for task to shut down");

        ep_evt_send
            .send(MetaNetEvt::Disconnected {
                remote_url: "".to_string(),
                con: mk_test_con(),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    // TODO no disconnect event is sent if the connection is force closed by us.
    #[tokio::test(flavor = "multi_thread")]
    async fn make_request_while_blocked() {
        let (mut ep_evt_send, _, _, _, host_stub, _, _, _) = setup().await;

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
                    data: wire::WireData(vec![]),
                }),
                respond: Box::new(|_| async move { () }.boxed().into()),
            })
            .await
            .unwrap();

        wait_for_condition(|| con_state.read().closed)
            .await
            .expect("Timed out waiting for the connection to be closed");

        assert!(con_state.read().closed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_call_request() {
        let (ep_evt_send, _, _, _, _, _, _, _) = setup().await;

        let request_data = vec![2, 7];

        let call_response = do_request(
            ep_evt_send,
            wire::Wire::Call(wire::Call {
                space: test_space(1),
                to_agent: test_agent(2),
                data: wire::WireData(request_data.clone()),
            }),
        )
        .await;

        let response_data = match call_response {
            wire::Wire::CallResp(res) => res.data.to_vec(),
            _ => panic!("Unexpected response"),
        };

        // Because the stub does an echo response
        assert_eq!(request_data, response_data);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_call_request_handles_error() {
        let (ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        let request_data = vec![2, 7];
        let call_response = do_request(
            ep_evt_send.clone(),
            wire::Wire::Call(wire::Call {
                space: test_space(1),
                to_agent: test_agent(2),
                data: wire::WireData(request_data.clone()),
            }),
        )
        .await;

        let reason = match call_response {
            wire::Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!("Other(\"a test error\")".to_string(), reason);

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_call_request_handles_shutdown() {
        let (ep_evt_send, _, _, host_receiver_stub, _, _, _, _) = setup().await;

        host_receiver_stub.abort();

        let request_data = vec![2, 7];
        let call_response = do_request(
            ep_evt_send,
            wire::Wire::Call(wire::Call {
                space: test_space(1),
                to_agent: test_agent(2),
                data: wire::WireData(request_data.clone()),
            }),
        )
        .await;

        let reason = match call_response {
            wire::Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!("GhostError(Disconnected)".to_string(), reason);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_peer_get_request() {
        let (ep_evt_send, _, _, _, _, _, _, _) = setup().await;

        let call_response = do_request(
            ep_evt_send,
            wire::Wire::PeerGet(wire::PeerGet {
                space: test_space(1),
                agent: test_agent(1),
            }),
        )
        .await;

        let agent_info_signed = match call_response {
            wire::Wire::PeerGetResp(res) => res.agent_info_signed,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(test_agent(1), agent_info_signed.unwrap().agent);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_peer_get_request_error() {
        let (ep_evt_send, _, _, _, host_stub, _, _, meta_net_task_finished) = setup().await;

        // Set up the error response so that when we make a request we get an error
        host_stub.fail_next_request();

        let call_response = do_request(
            ep_evt_send.clone(),
            wire::Wire::PeerGet(wire::PeerGet {
                space: test_space(1),
                agent: test_agent(1),
            }),
        )
        .await;

        let reason = match call_response {
            wire::Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!("Error getting agent: \"error for unimplemented KitsuneHost test behavior: method get_agent_info_signed of HostStub\"".to_string(), reason);

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn make_peer_query_request() {
        let (ep_evt_send, _, _, _, _, _, _, _) = setup().await;

        let response = do_request(
            ep_evt_send,
            wire::Wire::PeerQuery(wire::PeerQuery {
                space: test_space(1),
                basis_loc: DhtLocation::new(1),
            }),
        )
        .await;

        let peer_list = match response {
            wire::Wire::PeerQueryResp(r) => r.peer_list,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(8, peer_list.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_peer_query_request_error() {
        let (ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        // Set up the error response so that when we make a request we get an error
        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        let response = do_request(
            ep_evt_send.clone(),
            wire::Wire::PeerQuery(wire::PeerQuery {
                space: test_space(1),
                basis_loc: DhtLocation::new(1),
            }),
        )
        .await;

        let reason = match response {
            wire::Wire::Failure(f) => f.reason,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(
            "Error querying agents: Other(\"a test error\")".to_string(),
            reason
        );

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ignores_unexpected_request_payload() {
        let (mut ep_evt_send, _, _, _, _, _, _, meta_net_task_finished) = setup().await;

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
                respond: Box::new(|_| async move { () }.boxed().into()),
            })
            .await
            .unwrap();

        // Now check that we can still use the task to send/receive messages.
        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    // TODO no disconnect event is sent if the connection is force closed by us.
    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_while_blocked() {
        let (mut ep_evt_send, _, _, _, host_stub, _, _, _) = setup().await;

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
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: con,
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| con_state.read().closed)
            .await
            .expect("Timed out waiting for the connection to be closed");

        assert!(con_state.read().closed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_publish() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| !internal_stub.incoming_publish_calls.read().is_empty())
            .await
            .expect("Timed out waiting for a publish call");

        let args = internal_stub
            .incoming_publish_calls
            .read()
            .first()
            .unwrap()
            .clone();
        assert_eq!(test_space(1), args.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_publish_fails_to_forward() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, meta_net_task_finished) = setup().await;

        internal_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for a publish call error");

        assert_eq!(
            1,
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(internal_stub.incoming_publish_calls.read().is_empty());

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_publish_handles_shutdown() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, met_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(met_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_user() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            !internal_stub
                .incoming_delegate_broadcast_calls
                .read()
                .is_empty()
        })
        .await
        .expect("Timed out waiting for a publish call");

        let args = internal_stub
            .incoming_delegate_broadcast_calls
            .read()
            .first()
            .unwrap()
            .clone();
        assert_eq!(BroadcastData::User(test_agent(5).to_vec()), args.5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_user_fails_to_forward() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, meta_net_task_finished) = setup().await;

        internal_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for a publish call");

        assert_eq!(
            1,
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(internal_stub
            .incoming_delegate_broadcast_calls
            .read()
            .is_empty());

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_agent_info() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            !internal_stub
                .incoming_delegate_broadcast_calls
                .read()
                .is_empty()
        })
        .await
        .expect("Timed out waiting for a delegate broadcast");

        let args = internal_stub
            .incoming_delegate_broadcast_calls
            .read()
            .first()
            .unwrap()
            .clone();
        assert_eq!(BroadcastData::AgentInfo(mk_agent_info(6).await), args.5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_agent_info_fails_to_forward() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, meta_net_task_finished) = setup().await;

        internal_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for an error");

        assert_eq!(
            1,
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(internal_stub
            .incoming_delegate_broadcast_calls
            .read()
            .is_empty());

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_delegate_broadcast_user_handles_shutdown() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::DelegateBroadcast(wire::DelegateBroadcast {
                    space: test_space(1),
                    basis: Arc::new(KitsuneBasis::new(vec![0; 36])),
                    to_agent: test_agent(2),
                    mod_idx: 0,
                    mod_cnt: 0,
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_publish() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| !internal_stub.incoming_publish_calls.read().is_empty())
            .await
            .expect("Timed out waiting for a publish broadcast");

        let args = internal_stub
            .incoming_publish_calls
            .read()
            .first()
            .unwrap()
            .clone();
        assert_eq!(test_space(1), args.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_publish_fails_to_forward() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, meta_net_task_finished) = setup().await;

        internal_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for an error");

        assert_eq!(
            1,
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(internal_stub.incoming_publish_calls.read().is_empty());

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_publish_handles_shutdown() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::Publish {
                        source: test_agent(5),
                        op_hash_list: vec![],
                        context: Default::default(),
                    },
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_user() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| !host_receiver_stub.notify_calls.read().is_empty())
            .await
            .expect("Timed out waiting for a notify");

        assert_eq!(1, host_receiver_stub.notify_calls.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_user_fails_to_forward() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for an error");

        assert_eq!(
            1,
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(host_receiver_stub.notify_calls.read().is_empty());
        assert!(!meta_net_task_finished.load(Ordering::Acquire));

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_user_handles_shutdown() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub.abort();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::User(test_agent(5).to_vec()),
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_agent_info() {
        let (mut ep_evt_send, _, _, mut host_receiver_stub, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        let agent_info_evt = host_receiver_stub
            .next_event(Duration::from_millis(1000))
            .await;
        assert_eq!(1, agent_info_evt.peer_data.len());
        assert_eq!(
            mk_agent_info(6).await,
            agent_info_evt.peer_data.first().unwrap().clone()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_agent_info_fails_to_forward() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for a publish call");

        assert_eq!(
            1,
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );
        assert!(host_receiver_stub
            .put_agent_info_signed_calls
            .read()
            .is_empty());

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_broadcast_agent_info_handles_shutdown() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub.abort();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_gossip() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Gossip(wire::Gossip {
                    space: test_space(1),
                    data: wire::WireData(vec![1, 4, 6]),
                    module: GossipModuleType::ShardedRecent,
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| !internal_stub.incoming_gossip_calls.read().is_empty())
            .await
            .expect("Timed out waiting for incoming gossip");

        assert_eq!(1, internal_stub.incoming_gossip_calls.read().len());
        assert_eq!(
            vec![1, 4, 6],
            internal_stub
                .incoming_gossip_calls
                .read()
                .first()
                .clone()
                .unwrap()
                .3
                .to_vec()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_gossip_handles_error() {
        let (mut ep_evt_send, internal_stub, _, _, _, _, _, meta_net_task_finished) = setup().await;

        // Set up an error
        internal_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Gossip(wire::Gossip {
                    space: test_space(1),
                    data: wire::WireData(vec![1, 4, 6]),
                    module: GossipModuleType::ShardedRecent,
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| {
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for an error");

        assert_eq!(
            1,
            internal_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_gossip_handles_shutdown() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, _, meta_net_task_finished) =
            setup().await;

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Gossip(wire::Gossip {
                    space: test_space(1),
                    data: wire::WireData(vec![1, 4, 6]),
                    module: GossipModuleType::ShardedRecent,
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_fetch_op() {
        let (mut ep_evt_send, _, _, _, _, fetch_response_queue, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::FetchOp(wire::FetchOp {
                    fetch_list: vec![(test_space(1), vec![test_key_op(1), test_key_op(2)])],
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| fetch_response_queue.bytes_sent.load(Ordering::Acquire) == 6)
            .await
            .expect("Timed out waiting for op fetch");

        assert_eq!(6, fetch_response_queue.bytes_sent.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_fetch_op_fail_independently() {
        let (
            mut ep_evt_send,
            _,
            _,
            host_receiver_stub,
            _,
            fetch_response_queue,
            _,
            meta_net_task_finished,
        ) = setup().await;

        // The first call will fail, subsequent calls succeed
        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::FetchOp(wire::FetchOp {
                    fetch_list: vec![
                        (test_space(1), vec![test_key_op(1), test_key_op(2)]),
                        (test_space(2), vec![test_key_op(3), test_key_op(4)]),
                    ],
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| fetch_response_queue.bytes_sent.load(Ordering::Acquire) == 6)
            .await
            .expect("Timed out waiting for op fetch");

        // The list for the first space does not get sent due to an error fetching its op data but the second does succeed and gets sent
        assert_eq!(6, fetch_response_queue.bytes_sent.load(Ordering::Acquire));

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_fetch_op_handles_shutdown() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub.abort();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::FetchOp(wire::FetchOp {
                    fetch_list: vec![(test_space(1), vec![test_key_op(1), test_key_op(2)])],
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_push_op_data() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, fetch_pool, _) = setup().await;

        fetch_pool.push(test_req_op(1, None, test_source(2)));
        assert_eq!(1, fetch_pool.len());

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PushOpData(wire::PushOpData {
                    op_data_list: vec![(
                        test_space(1),
                        vec![PushOpItem {
                            op_data: KitsuneOpData::new(vec![1, 4, 10]),
                            region: None,
                        }],
                    )],
                }),
            })
            .await
            .unwrap();

        wait_for_condition(|| fetch_pool.is_empty())
            .await
            .expect("Timed out waiting for op push");

        assert!(fetch_pool.is_empty());
        assert_eq!(1, host_receiver_stub.receive_ops_calls.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_push_op_data_fails_independently_on_op_hash_error() {
        let (mut ep_evt_send, _, _, host_receiver_stub, host_stub, _, fetch_pool, _) =
            setup().await;

        host_stub.fail_next_request();

        fetch_pool.push(test_req_op(0, None, test_source(2)));
        assert_eq!(1, fetch_pool.len());

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PushOpData(wire::PushOpData {
                    op_data_list: vec![
                        (
                            test_space(1),
                            vec![PushOpItem {
                                op_data: KitsuneOpData::new(vec![0, 4, 10]),
                                region: None,
                            }],
                        ),
                        (
                            test_space(1),
                            vec![PushOpItem {
                                op_data: KitsuneOpData::new(vec![0, 3, 90]),
                                region: None,
                            }],
                        ),
                    ],
                }),
            })
            .await
            .unwrap();

        // Check that there was an error
        wait_for_condition(|| host_stub.get_fail_count() != 0)
            .await
            .expect("Timed out waiting for an error");

        assert_eq!(1, host_stub.get_fail_count());

        // and also a successful op push
        wait_for_condition(|| {
            fetch_pool.is_empty() && !host_receiver_stub.receive_ops_calls.read().is_empty()
        })
        .await
        .expect("Timed out waiting for op push");

        assert!(fetch_pool.is_empty());
        assert_eq!(1, host_receiver_stub.receive_ops_calls.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_push_op_data_fails_independently_on_receive_ops_error() {
        holochain_trace::test_run().unwrap();

        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, fetch_pool, _) = setup().await;

        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        fetch_pool.push(test_req_op(0, None, test_source(2)));
        fetch_pool.push(test_req_op(1, None, test_source(3)));
        assert_eq!(2, fetch_pool.len());

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PushOpData(wire::PushOpData {
                    op_data_list: vec![
                        (
                            test_space(1),
                            vec![PushOpItem {
                                op_data: KitsuneOpData::new(vec![0, 4, 10]),
                                region: None,
                            }],
                        ),
                        (
                            test_space(1),
                            vec![PushOpItem {
                                op_data: KitsuneOpData::new(vec![1, 3, 90]),
                                region: None,
                            }],
                        ),
                    ],
                }),
            })
            .await
            .unwrap();

        // Check that there was an error
        wait_for_condition(|| {
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
                != 0
        })
        .await
        .expect("Timed out waiting for an error");

        assert_eq!(
            1,
            host_receiver_stub
                .respond_with_error_count
                .load(Ordering::Acquire)
        );

        // Manually drop the item from the pool that we failed to receive
        fetch_pool.remove(&test_key_op(0));

        // and also a successful op push
        wait_for_condition(|| {
            fetch_pool.is_empty() && !host_receiver_stub.receive_ops_calls.read().is_empty()
        })
        .await
        .expect("Timed out waiting for op push");

        assert!(fetch_pool.is_empty());
        assert_eq!(1, host_receiver_stub.receive_ops_calls.read().len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_push_op_data_handles_shutdown_on_receive_ops() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, fetch_pool, meta_net_task_finished) =
            setup().await;

        fetch_pool.push(test_req_op(0, None, test_source(2)));
        assert_eq!(1, fetch_pool.len());

        host_receiver_stub.abort();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PushOpData(wire::PushOpData {
                    op_data_list: vec![(
                        test_space(1),
                        vec![PushOpItem {
                            op_data: KitsuneOpData::new(vec![0, 4, 10]),
                            region: None,
                        }],
                    )],
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_push_op_data_handles_shutdown_on_resolve() {
        let (mut ep_evt_send, _, internal_sender, _, _, _, fetch_pool, meta_net_task_finished) =
            setup().await;

        fetch_pool.push(test_req_op(0, None, test_source(2)));
        assert_eq!(1, fetch_pool.len());

        internal_sender
            .ghost_actor_shutdown_immediate()
            .await
            .unwrap();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PushOpData(wire::PushOpData {
                    op_data_list: vec![(
                        test_space(1),
                        vec![PushOpItem {
                            op_data: KitsuneOpData::new(vec![0, 4, 10]),
                            region: None,
                        }],
                    )],
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_peer_unsolicited() {
        let (mut ep_evt_send, _, _, mut host_receiver_stub, _, _, _, _) = setup().await;

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PeerUnsolicited(wire::PeerUnsolicited {
                    peer_list: vec![mk_agent_info(1).await, mk_agent_info(2).await],
                }),
            })
            .await
            .unwrap();

        // Wait for both agent infos to be received
        for i in 1..3 {
            assert_eq!(
                mk_agent_info(i).await,
                host_receiver_stub
                    .next_event(Duration::from_secs(1))
                    .await
                    .peer_data
                    .first()
                    .unwrap()
                    .clone()
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_peer_unsolicited_fails_independently() {
        let (mut ep_evt_send, _, _, mut host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        // Set up an error for the first call
        host_receiver_stub
            .respond_with_error
            .store(true, Ordering::SeqCst);

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PeerUnsolicited(wire::PeerUnsolicited {
                    // Send two agent infos
                    peer_list: vec![mk_agent_info(1).await, mk_agent_info(2).await],
                }),
            })
            .await
            .unwrap();

        // Expect only the second agent info
        assert_eq!(
            mk_agent_info(2).await,
            host_receiver_stub
                .next_event(Duration::from_secs(1))
                .await
                .peer_data
                .first()
                .unwrap()
                .clone()
        );

        verify_task_live(ep_evt_send, meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_notify_peer_unsolicited_handles_shutdown() {
        let (mut ep_evt_send, _, _, host_receiver_stub, _, _, _, meta_net_task_finished) =
            setup().await;

        host_receiver_stub.abort();

        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::PeerUnsolicited(wire::PeerUnsolicited {
                    peer_list: vec![mk_agent_info(1).await, mk_agent_info(2).await],
                }),
            })
            .await
            .unwrap();

        wait_and_assert_shutdown(meta_net_task_finished).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ignores_unexpected_notify_payload() {
        let (mut ep_evt_send, _, _, mut host_receiver_stub, _, _, _, _) = setup().await;

        // Send a notification with a payload that is not expected.
        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data: wire::Wire::PeerQuery(wire::PeerQuery {
                    space: test_space(1),
                    basis_loc: DhtLocation::new(1),
                }),
            })
            .await
            .unwrap();

        // Now check that we can still use the task to send notify messages.
        ep_evt_send
            .send(MetaNetEvt::Notify {
                remote_url: "".to_string(),
                con: mk_test_con(),
                data: wire::Wire::Broadcast(wire::Broadcast {
                    space: test_space(1),
                    to_agent: test_agent(2),
                    data: BroadcastData::AgentInfo(mk_agent_info(6).await),
                }),
            })
            .await
            .unwrap();

        let agent_info_evt = host_receiver_stub
            .next_event(Duration::from_millis(1000))
            .await;
        assert_eq!(1, agent_info_evt.peer_data.len());
        assert_eq!(
            mk_agent_info(6).await,
            agent_info_evt.peer_data.first().unwrap().clone()
        );
    }

    async fn setup() -> (
        Sender<MetaNetEvt>,
        InternalStub,
        GhostSender<Internal>,
        HostReceiverStub,
        Arc<HostStub>,
        FetchResponseQueue<FetchResponseConfig>,
        FetchPool,
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
            host_stub.clone().legacy(host_sender),
            Default::default(),
            fetch_pool.clone(),
            fetch_response_queue.clone(),
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
            fetch_response_queue,
            fetch_pool,
            meta_net_task_finished,
        )
    }

    async fn do_request(mut ep_evt_send: Sender<MetaNetEvt>, data: wire::Wire) -> wire::Wire {
        let (send_res, read_res) = futures::channel::oneshot::channel();

        ep_evt_send
            .send(MetaNetEvt::Request {
                remote_url: "".to_string(),
                con: mk_test_con_with_id(1),
                data,
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

        tokio::time::timeout(Duration::from_secs(1), read_res)
            .await
            .expect("Timed out while waiting for a response")
            .unwrap()
    }

    async fn verify_task_live(
        ep_evt_send: Sender<MetaNetEvt>,
        meta_net_task_finished: Arc<AtomicBool>,
    ) {
        assert!(!meta_net_task_finished.load(Ordering::Acquire));

        let response = do_request(
            ep_evt_send,
            wire::Wire::PeerQuery(wire::PeerQuery {
                space: test_space(1),
                basis_loc: DhtLocation::new(1),
            }),
        )
        .await;

        let peer_list = match response {
            wire::Wire::PeerQueryResp(r) => r.peer_list,
            r => panic!("Unexpected response - {:?}", r),
        };

        assert_eq!(8, peer_list.len());
        assert!(!meta_net_task_finished.load(Ordering::Acquire));
    }

    async fn wait_and_assert_shutdown(meta_net_task_finished: Arc<AtomicBool>) {
        wait_for_condition(|| meta_net_task_finished.load(Ordering::Acquire))
            .await
            .expect("Timed out waiting for shutdown");

        assert!(meta_net_task_finished.load(Ordering::Acquire));
    }

    async fn wait_for_condition(
        cond: impl Fn() -> bool,
    ) -> Result<(), tokio::time::error::Elapsed> {
        tokio::time::timeout(Duration::from_millis(1000), async {
            while !cond() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
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
        NodeCert::from(Arc::new(vec![i; 32].try_into().unwrap()))
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
