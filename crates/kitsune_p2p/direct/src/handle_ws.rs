use crate::types::handle::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::sink::SinkExt;
use futures::stream::{BoxStream, StreamExt};
use kitsune_p2p_direct_api::kd_entry::KdEntryBinary;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Create a new KitsuneDirect controller handle over the websocket channel.
pub async fn new_handle_ws(
    ws_addr: std::net::SocketAddr,
    _connect_passphrase: sodoken::BufRead,
) -> KdResult<(KdHnd, KdHndEvtStream)> {
    let url = format!("ws://{}", ws_addr);

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(KdError::other)?;

    let first = match ws_stream.next().await {
        Some(Ok(tungstenite::Message::Text(json))) => serde_json::from_str(&json),
        Some(Ok(tungstenite::Message::Binary(json))) => serde_json::from_slice(&json),
        oth => return Err(format!("unexpected: {:?}", oth).into()),
    }
    .map_err(KdError::other)?;
    match first {
        KdApi::HelloReq { msg_id, .. } => {
            let auth = vec![1, 2, 3, 4].into_boxed_slice().into();
            let api = KdApi::HelloRes { msg_id, auth };
            let api = api.to_string();
            let api = tungstenite::Message::Text(api);
            ws_stream.send(api).await.map_err(KdError::other)?;
        }
        oth => return Err(format!("unexpected: {:?}", oth).into()),
    }

    let (ws_write, ws_read) = ws_stream.split();
    let (ws_write_snd, ws_write_rcv) = futures::channel::mpsc::channel(32);
    tokio::task::spawn(ws_write_rcv.forward(ws_write));

    let tuning_params = KitsuneP2pTuningParams::default();

    let logic_chan = LogicChan::new(tuning_params.concurrent_limit_per_thread);
    let lhnd = logic_chan.handle().clone();

    let hnd = Hnd::new(ws_write_snd);

    let lhnd2 = lhnd.clone();
    lhnd2
        .capture_logic(handle_ws_recv(
            tuning_params,
            ws_read.boxed(),
            hnd.clone(),
            lhnd,
        ))
        .await
        .map_err(KdError::other)?;

    let hnd = KdHnd(hnd);

    Ok((hnd, Box::new(logic_chan)))
}

// -- private -- //

static MSG_ID: AtomicU64 = AtomicU64::new(1);
fn new_msg_id() -> String {
    use rand::Rng;
    let rx: u64 = rand::thread_rng().gen();
    let nx = MSG_ID.fetch_add(1, Ordering::Relaxed);
    format!("{}{}", rx, nx)
}

type WsWrite = futures::channel::mpsc::Sender<Result<tungstenite::Message, tungstenite::Error>>;

struct HndInner {
    responses: HashMap<String, tokio::sync::oneshot::Sender<KdResult<KdApi>>>,
    ws_write: WsWrite,
}

#[derive(Clone)]
struct Hnd {
    uniq: Uniq,
    inner: Share<HndInner>,
}

impl Hnd {
    pub fn new(ws_write: WsWrite) -> Arc<Self> {
        Arc::new(Self {
            uniq: Uniq::default(),
            inner: Share::new(HndInner {
                responses: HashMap::new(),
                ws_write,
            }),
        })
    }

    fn request(
        &self,
        api: KdApi,
    ) -> impl std::future::Future<Output = KdResult<KdApi>> + 'static + Send {
        let msg_id = api.msg_id().to_string();
        let api = api.to_string();
        let api = tungstenite::Message::Text(api);
        let (s, r) = tokio::sync::oneshot::channel();
        let ws_write = self.inner.share_mut(move |i, _| {
            i.responses.insert(msg_id, s);
            Ok(i.ws_write.clone())
        });
        async move {
            ws_write
                .map_err(KdError::other)?
                .send(Ok(api))
                .await
                .map_err(KdError::other)?;
            r.await.map_err(KdError::other)?
        }
    }
}

impl AsKdHnd for Hnd {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        self.inner.close();
        async move {}.boxed()
    }

    fn keypair_get_or_create_tagged(&self, tag: &str) -> BoxFuture<'static, KdResult<KdHash>> {
        let msg_id = new_msg_id();
        let api = KdApi::KeypairGetOrCreateTaggedReq {
            msg_id,
            tag: tag.to_string(),
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::KeypairGetOrCreateTaggedRes { pub_key, .. }) => Ok(pub_key),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn app_join(&self, root: KdHash, agent: KdHash) -> BoxFuture<'static, KdResult<()>> {
        let msg_id = new_msg_id();
        let api = KdApi::AppJoinReq {
            msg_id,
            root,
            agent,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::AppJoinRes { .. }) => Ok(()),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn app_leave(&self, root: KdHash, agent: KdHash) -> BoxFuture<'static, KdResult<()>> {
        let msg_id = new_msg_id();
        let api = KdApi::AppLeaveReq {
            msg_id,
            root,
            agent,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::AppLeaveRes { .. }) => Ok(()),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn agent_info_store(&self, agent_info: KdAgentInfo) -> BoxFuture<'static, KdResult<()>> {
        let msg_id = new_msg_id();
        let api = KdApi::AgentInfoStoreReq { msg_id, agent_info };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::AgentInfoStoreRes { .. }) => Ok(()),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn agent_info_get(
        &self,
        root: KdHash,
        agent: KdHash,
    ) -> BoxFuture<'static, KdResult<KdAgentInfo>> {
        let msg_id = new_msg_id();
        let api = KdApi::AgentInfoGetReq {
            msg_id,
            root,
            agent,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::AgentInfoGetRes { agent_info, .. }) => Ok(agent_info),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn agent_info_query(&self, root: KdHash) -> BoxFuture<'static, KdResult<Vec<KdAgentInfo>>> {
        let msg_id = new_msg_id();
        let api = KdApi::AgentInfoQueryReq { msg_id, root };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::AgentInfoQueryRes {
                    agent_info_list, ..
                }) => Ok(agent_info_list),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn is_authority(
        &self,
        root: KdHash,
        agent: KdHash,
        basis: KdHash,
    ) -> BoxFuture<'static, KdResult<bool>> {
        let msg_id = new_msg_id();
        let api = KdApi::IsAuthorityReq {
            msg_id,
            root,
            agent,
            basis,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::IsAuthorityRes { is_authority, .. }) => Ok(is_authority),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn message_send(
        &self,
        root: KdHash,
        to_agent: KdHash,
        from_agent: KdHash,
        content: serde_json::Value,
        binary: KdEntryBinary,
    ) -> BoxFuture<'static, KdResult<()>> {
        let msg_id = new_msg_id();
        let api = KdApi::MessageSendReq {
            msg_id,
            root,
            to_agent,
            from_agent,
            content,
            binary,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::MessageSendRes { .. }) => Ok(()),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn entry_author(
        &self,
        root: KdHash,
        author: KdHash,
        content: KdEntryContent,
        binary: KdEntryBinary,
    ) -> BoxFuture<'static, KdResult<KdEntrySigned>> {
        let msg_id = new_msg_id();
        let api = KdApi::EntryAuthorReq {
            msg_id,
            root,
            author,
            content,
            binary,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::EntryAuthorRes { entry_signed, .. }) => Ok(entry_signed),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    /// Get a specific entry
    fn entry_get(
        &self,
        root: KdHash,
        agent: KdHash,
        hash: KdHash,
    ) -> BoxFuture<'static, KdResult<KdEntrySigned>> {
        let msg_id = new_msg_id();
        let api = KdApi::EntryGetReq {
            msg_id,
            root,
            agent,
            hash,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::EntryGetRes { entry_signed, .. }) => Ok(entry_signed),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }

    fn entry_get_children(
        &self,
        root: KdHash,
        parent: KdHash,
        kind: Option<String>,
    ) -> BoxFuture<'static, KdResult<Vec<KdEntrySigned>>> {
        let msg_id = new_msg_id();
        let api = KdApi::EntryGetChildrenReq {
            msg_id,
            root,
            parent,
            kind,
        };
        let api = self.request(api);
        async move {
            match api.await {
                Ok(KdApi::EntryGetChildrenRes {
                    entry_signed_list, ..
                }) => Ok(entry_signed_list),
                oth => Err(format!("unexpected: {:?}", oth).into()),
            }
        }
        .boxed()
    }
}

async fn handle_ws_recv(
    tuning_params: KitsuneP2pTuningParams,
    ws_read: BoxStream<'static, Result<tungstenite::Message, tungstenite::Error>>,
    hnd: Arc<Hnd>,
    lhnd: LogicChanHandle<KdHndEvt>,
) {
    let hnd = &hnd;
    let lhnd = &lhnd;

    ws_read
        .for_each_concurrent(
            tuning_params.concurrent_limit_per_thread,
            move |evt| async move {
                let api: KdApi = match match evt {
                    Ok(tungstenite::Message::Text(json)) => serde_json::from_str(&json),
                    Ok(tungstenite::Message::Binary(json)) => serde_json::from_slice(&json),
                    Ok(tungstenite::Message::Close(_)) => {
                        tracing::debug!("kdhnd recv ws close");
                        return;
                    }
                    evt => {
                        tracing::warn!(?evt, "invalid websocket message");
                        return;
                    }
                } {
                    Ok(api) => api,
                    Err(err) => {
                        tracing::warn!(?err, "failed to parse json");
                        return;
                    }
                };
                if let KdApi::MessageRecvEvt {
                    root,
                    to_agent,
                    content,
                    binary,
                } = api
                {
                    if let Err(err) = lhnd
                        .emit(KdHndEvt::Message {
                            root,
                            to_agent,
                            content,
                            binary,
                        })
                        .await
                    {
                        tracing::error!(?err, "error emitting incoming message");
                    }
                    return;
                }
                if api.is_res() {
                    if let Ok(Some(snd)) = hnd
                        .inner
                        .share_mut(|i, _| Ok(i.responses.remove(api.msg_id())))
                    {
                        let _ = snd.send(Ok(api));
                    }
                } else {
                    tracing::error!("unexpected: {:?}", api);
                }
            },
        )
        .await;
    tracing::debug!("kdhnd recv shutdown");
}
