use crate::event::{
    FetchOpDataEvtQuery, KitsuneP2pEvent, KitsuneP2pEventHandlerResult, PutAgentInfoSignedEvt,
};
use crate::spawn::actor::{KAgent, KSpace};
use crate::test_util::data::mk_agent_info;
use crate::types::event::Payload;
use crate::{KOp, KitsuneP2pError};
use futures::channel::mpsc::{channel, Receiver};
use futures::{FutureExt, SinkExt, StreamExt};
use ghost_actor::GhostRespond;
use kitsune_p2p_types::bin_types::KitsuneOpData;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::AbortHandle;
use tokio::time::error::Elapsed;

pub struct LegacyHostStub {
    pub respond_with_error: Arc<AtomicBool>,
    pub respond_with_error_count: Arc<AtomicUsize>,

    pub put_agent_info_signed_calls: Arc<parking_lot::RwLock<Vec<PutAgentInfoSignedEvt>>>,
    pub notify_calls: Arc<parking_lot::RwLock<Vec<(KSpace, KAgent, Payload)>>>,
    pub receive_ops_calls:
        Arc<parking_lot::RwLock<Vec<(KSpace, Vec<KOp>, Option<kitsune_p2p_fetch::FetchContext>)>>>,

    put_events: Receiver<PutAgentInfoSignedEvt>,
    abort_handle: AbortHandle,
}

impl LegacyHostStub {
    pub fn start(mut host_receiver: Receiver<KitsuneP2pEvent>) -> Self {
        let (mut sender, receiver) = channel(10);

        let put_agent_info_signed_calls = Arc::new(parking_lot::RwLock::new(Vec::new()));
        let notify_calls = Arc::new(parking_lot::RwLock::new(Vec::new()));
        let receive_ops_calls = Arc::new(parking_lot::RwLock::new(Vec::new()));

        let respond_with_error = Arc::new(AtomicBool::new(false));
        let respond_with_error_count = Arc::new(AtomicUsize::new(0));

        let handle = tokio::spawn({
            let task_respond_with_error = respond_with_error.clone();
            let task_respond_with_error_count = respond_with_error_count.clone();

            let task_put_agent_info_signed_calls = put_agent_info_signed_calls.clone();
            let task_notify_calls = notify_calls.clone();
            let task_receive_ops_calls = receive_ops_calls.clone();

            async move {
                while let Some(evt) = host_receiver.next().await {
                    match evt {
                        KitsuneP2pEvent::PutAgentInfoSigned { input, respond, .. } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            task_put_agent_info_signed_calls.write().push(input.clone());
                            sender.send(input).await.unwrap();

                            respond
                                .unwrap()
                                .respond(Ok(async move { Ok(()) }.boxed().into()));
                        }
                        KitsuneP2pEvent::Call {
                            payload, respond, ..
                        } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            // An echo response, no need for anything fancy here
                            respond
                                .unwrap()
                                .respond(Ok(async move { Ok(payload.to_vec()) }.boxed().into()));
                        }
                        KitsuneP2pEvent::QueryAgents { input, respond, .. } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            let len = input.limit.unwrap();

                            respond.unwrap().respond(Ok(async move {
                                let mut agents = vec![];
                                for i in 0..len {
                                    agents.push(mk_agent_info(i as u8).await);
                                }

                                Ok(agents)
                            }
                            .boxed()
                            .into()))
                        }
                        KitsuneP2pEvent::QueryPeerDensity { .. } => {}
                        KitsuneP2pEvent::Notify {
                            space,
                            to_agent,
                            payload,
                            respond,
                            ..
                        } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            task_notify_calls.write().push((space, to_agent, payload));

                            respond
                                .unwrap()
                                .respond(Ok(async move { Ok(()) }.boxed().into()))
                        }
                        KitsuneP2pEvent::FetchOpData { input, respond, .. } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            match input.query {
                                FetchOpDataEvtQuery::Hashes { op_hash_list, .. } => {
                                    let response = op_hash_list
                                        .into_iter()
                                        // TODO why are we responding with hashes when they are part of the input? It's an atomic
                                        //      operation in the sense that you get everything or an error so there is no matching to be done.
                                        .map(|h| (h, KitsuneOpData::new(vec![1, 2, 3])))
                                        .collect();

                                    respond
                                        .unwrap()
                                        .respond(Ok(async move { Ok(response) }.boxed().into()))
                                }
                                _ => {
                                    respond.unwrap().respond(Ok(async move {
                                        Err(KitsuneP2pError::other("a test error"))
                                    }
                                    .boxed()
                                    .into()));
                                }
                            }
                        }
                        KitsuneP2pEvent::ReceiveOps {
                            space,
                            ops,
                            context,
                            respond,
                            ..
                        } => {
                            let respond = maybe_respond_error(
                                task_respond_with_error.clone(),
                                task_respond_with_error_count.clone(),
                                respond,
                            );
                            if respond.is_none() {
                                continue;
                            }

                            task_receive_ops_calls.write().push((space, ops, context));

                            respond
                                .unwrap()
                                .respond(Ok(async move { Ok(()) }.boxed().into()))
                        }
                        _ => panic!("Unexpected event - {:?}", evt),
                    }
                }
            }
        });

        LegacyHostStub {
            respond_with_error,
            respond_with_error_count,
            put_agent_info_signed_calls,
            notify_calls,
            receive_ops_calls,
            put_events: receiver,
            abort_handle: handle.abort_handle(),
        }
    }

    pub async fn next_event(&mut self, timeout: Duration) -> PutAgentInfoSignedEvt {
        tokio::time::timeout(timeout, self.put_events.next())
            .await
            .unwrap()
            .unwrap()
    }

    pub async fn try_next_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<PutAgentInfoSignedEvt>, Elapsed> {
        tokio::time::timeout(timeout, self.put_events.next()).await
    }

    pub fn abort(&self) {
        self.abort_handle.abort();
    }
}

fn maybe_respond_error<T>(
    task_respond_with_error: Arc<AtomicBool>,
    count: Arc<AtomicUsize>,
    respond: GhostRespond<KitsuneP2pEventHandlerResult<T>>,
) -> Option<GhostRespond<KitsuneP2pEventHandlerResult<T>>> {
    if let Ok(true) =
        task_respond_with_error.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
    {
        count.fetch_add(1, Ordering::SeqCst);
        respond.respond(Ok(
            async move { Err(KitsuneP2pError::other("a test error")) }
                .boxed()
                .into(),
        ));
        None
    } else {
        Some(respond)
    }
}
