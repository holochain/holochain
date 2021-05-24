use crate::dependencies::tracing;
use crate::*;

use futures::future::{BoxFuture, FutureExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use kitsune_p2p_direct_api::KdApi;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::tx2::tx2_utils::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use types::srv::*;

/// create a new KdSrv instance on localhost for given port
pub async fn new_srv(
    tuning_params: KitsuneP2pTuningParams,
    port: u16,
) -> KitsuneResult<(KdSrv, KdSrvEvtStream)> {
    let logic_chan = LogicChan::new(tuning_params.concurrent_limit_per_thread);
    let lhnd = logic_chan.handle().clone();
    let kdsrv = Srv::new(port, lhnd).await?;

    let kdsrv = KdSrv(kdsrv);

    Ok((kdsrv, Box::new(logic_chan)))
}

// -- private -- //

type WsWrite = futures::channel::mpsc::Sender<Result<tungstenite::Message, tungstenite::Error>>;

struct SrvInner {
    addr: SocketAddr,
    lhnd: LogicChanHandle<KdSrvEvt>,
    websockets: HashMap<Uniq, WsWrite>,
}

#[derive(Clone)]
struct Srv {
    uniq: Uniq,
    inner: Share<SrvInner>,
}

impl Srv {
    pub async fn new(port: u16, lhnd: LogicChanHandle<KdSrvEvt>) -> KitsuneResult<Arc<Self>> {
        // kdirect only binds to local host for security reasons
        let addr = SocketAddr::from(([127, 0, 0, 1], port));

        let kdsrv = Arc::new(Self {
            uniq: Uniq::default(),
            inner: Share::new(SrvInner {
                addr,
                lhnd,
                websockets: HashMap::new(),
            }),
        });

        let srv_ref = kdsrv.clone();
        let server = hyper::Server::bind(&addr).serve(hyper::service::make_service_fn(move |_| {
            let srv_ref = srv_ref.clone();
            async move {
                let srv_ref = srv_ref.clone();

                KitsuneResult::Ok(hyper::service::service_fn(move |r| {
                    let srv_ref = srv_ref.clone();

                    async move { srv_ref.handle_incoming(r).await }
                }))
            }
        }));

        let addr = server.local_addr();
        kdsrv.inner.share_mut(move |i, _| {
            i.addr = addr;
            Ok(())
        })?;

        tokio::task::spawn(async move {
            if let Err(err) = server.await {
                tracing::error!(?err, "KdSrv error");
            }
        });

        Ok(kdsrv)
    }

    async fn handle_incoming(
        &self,
        req: hyper::Request<hyper::Body>,
    ) -> KitsuneResult<hyper::Response<hyper::Body>> {
        if req.headers().contains_key(hyper::header::UPGRADE) {
            // handle as a websocket
            let res = tungstenite::handshake::server::create_response_with_body(&req, || {
                hyper::Body::empty()
            })
            .map_err(KitsuneError::other)?;

            self.register_websocket(req).await?;

            Ok(res)
        } else {
            // handle as an http request
            let (r_snd, r_rcv) = tokio::sync::oneshot::channel();

            let uri = req.uri().to_string();
            let method = req.method().to_string();
            let headers = req
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
                .collect();
            let body = hyper::body::to_bytes(req)
                .await
                .map_err(KitsuneError::other)?;
            let body = body.as_ref().to_vec();
            let respond_cb: HttpRespondCb = Box::new(move |resp| {
                async move {
                    let _ = r_snd.send(resp);
                    Ok(())
                }
                .boxed()
            });

            let evt = KdSrvEvt::HttpRequest {
                uri,
                method,
                headers,
                body,
                respond_cb,
            };

            self.inner
                .share_mut(move |i, _| Ok(i.lhnd.emit(evt)))?
                .await?;

            let resp = r_rcv.await.map_err(KitsuneError::other)??;
            let body = hyper::Body::from(resp.body);
            let mut out = hyper::Response::new(body);
            *out.status_mut() =
                hyper::StatusCode::from_u16(resp.status).map_err(KitsuneError::other)?;
            for (k, v) in resp.headers {
                let k = hyper::header::HeaderName::from_lowercase(k.to_lowercase().as_bytes())
                    .map_err(KitsuneError::other)?;
                let v = hyper::header::HeaderValue::from_bytes(&v).map_err(KitsuneError::other)?;
                out.headers_mut().insert(k, v);
            }

            Ok(out)
        }
    }

    async fn register_websocket(&self, mut req: hyper::Request<hyper::Body>) -> KitsuneResult<()> {
        let inner = self.inner.clone();
        tokio::task::spawn(async move {
            let socket = hyper::upgrade::on(&mut req)
                .await
                .map_err(KitsuneError::other)?;

            let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
                socket,
                tokio_tungstenite::tungstenite::protocol::Role::Server,
                None,
            )
            .await;

            let (ws_write, mut ws_read) = ws_stream.split();
            let (ws_write_snd, ws_write_rcv) = futures::channel::mpsc::channel(32);
            tokio::task::spawn(ws_write_rcv.forward(ws_write));

            let mut ws_write_snd2 = ws_write_snd.clone();
            let uniq = Uniq::default();
            let lhnd = inner.share_mut(move |i, _| {
                i.websockets.insert(uniq, ws_write_snd);
                Ok(i.lhnd.clone())
            })?;

            let lhnd2 = lhnd.clone();
            tokio::task::spawn(async move {
                while let Some(msg) = ws_read.next().await {
                    let api = match msg {
                        Ok(tungstenite::Message::Text(json)) => {
                            serde_json::from_str(&json).map_err(KdError::other)
                        }
                        Ok(tungstenite::Message::Binary(json)) => {
                            serde_json::from_slice(&json).map_err(KdError::other)
                        }
                        oth => Err(KdError::other(format!("unexpected: {:?}", oth))),
                    };
                    let api = match api {
                        Ok(api) => api,
                        Err(err) => {
                            let err = format!("{:?}", err);
                            let err = KdApi::ErrorRes {
                                msg_id: "".to_string(),
                                reason: err,
                            };
                            let err = err.to_string();
                            let err = tungstenite::Message::Text(err);
                            if ws_write_snd2.send(Ok(err)).await.is_err() {
                                break;
                            }
                            continue;
                        }
                    };
                    if lhnd2
                        .emit(KdSrvEvt::WebsocketMessage {
                            con: uniq,
                            data: api,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });

            lhnd.emit(KdSrvEvt::WebsocketConnected { con: uniq })
                .await?;

            KitsuneResult::Ok(())
        });

        Ok(())
    }
}

impl AsKdSrv for Srv {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn close(&self) -> BoxFuture<'static, ()> {
        self.inner.close();
        async move {}.boxed()
    }

    fn local_addr(&self) -> KitsuneResult<std::net::SocketAddr> {
        self.inner.share_mut(|i, _| Ok(i.addr))
    }

    fn websocket_broadcast(&self, data: KdApi) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.inner.clone();
        async move {
            let data = data.to_string();
            let data = tungstenite::Message::Text(data);
            let all = inner.share_mut(|i, _| {
                let mut all = Vec::new();
                for ws in i.websockets.values() {
                    let mut ws = ws.clone();
                    let data = data.clone();
                    all.push(async move { ws.send(Ok(data)).await });
                }
                Ok(all)
            })?;
            futures::future::try_join_all(all)
                .await
                .map_err(KitsuneError::other)?;
            Ok(())
        }
        .boxed()
    }

    fn websocket_send(&self, con: Uniq, data: KdApi) -> BoxFuture<'static, KitsuneResult<()>> {
        let inner = self.inner.clone();
        async move {
            let data = data.to_string();
            let data = tungstenite::Message::Text(data);
            inner
                .share_mut(move |i, _| {
                    if let Some(ws) = i.websockets.get(&con) {
                        let mut ws = ws.clone();
                        Ok(async move { ws.send(Ok(data)).await })
                    } else {
                        Err("no such websocket connection".into())
                    }
                })?
                .await
                .map_err(KitsuneError::other)?;
            Ok(())
        }
        .boxed()
    }
}
