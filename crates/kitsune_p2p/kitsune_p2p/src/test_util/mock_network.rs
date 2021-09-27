use std::collections::HashMap;
use std::sync::Arc;

use futures::FutureExt;
use futures::StreamExt;
use kitsune_p2p_types::tx2::tx2_adapter::test_utils::*;
use kitsune_p2p_types::tx2::tx2_adapter::*;
use kitsune_p2p_types::tx2::tx2_utils::PoolBuf;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::Tx2Cert;

use crate::wire;

pub type FromKitsuneMockChannelTx = tokio::sync::mpsc::Sender<KitsuneMock>;
pub type FromKitsuneMockChannelRx = tokio::sync::mpsc::Receiver<KitsuneMock>;
pub type ToKitsuneMockChannelTx = tokio::sync::mpsc::Sender<KitsuneMock>;
pub type ToKitsuneMockChannelRx =
    Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Receiver<KitsuneMock>>>>;

// const ADDR: &'static str =
//     "kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-";
#[derive(Debug)]
pub struct KitsuneMock {
    msg: KitsuneMockMsg,
    respond: Option<tokio::sync::oneshot::Sender<KitsuneMockMsg>>,
    cert: Tx2Cert,
    url: TxUrl,
}

#[derive(Debug)]
pub struct KitsuneMockMsg {
    msg: wire::Wire,
    id: MsgId,
    buf: PoolBuf,
}

#[derive(Debug)]
pub struct KitsuneMockRespond {
    respond: tokio::sync::oneshot::Sender<KitsuneMockMsg>,
    id: MsgId,
    buf: PoolBuf,
}

pub fn to_kitsune_channel(buffer: usize) -> (ToKitsuneMockChannelTx, ToKitsuneMockChannelRx) {
    let (tx, rx) = tokio::sync::mpsc::channel(buffer);
    (tx, Arc::new(parking_lot::Mutex::new(Some(rx))))
}

impl KitsuneMockRespond {
    pub fn respond(self, msg: wire::Wire) {
        let Self { respond, id, buf } = self;
        respond.send(KitsuneMockMsg { msg, id, buf }).unwrap();
    }
}

impl KitsuneMock {
    pub fn request(
        id: MsgId,
        cert: Tx2Cert,
        url: TxUrl,
        msg: wire::Wire,
        respond: tokio::sync::oneshot::Sender<KitsuneMockMsg>,
    ) -> Self {
        Self {
            msg: KitsuneMockMsg {
                msg,
                id,
                buf: PoolBuf::new(),
            },
            respond: Some(respond),
            cert,
            url,
        }
    }
    pub fn notify(id: MsgId, cert: Tx2Cert, url: TxUrl, msg: wire::Wire) -> Self {
        Self {
            msg: KitsuneMockMsg {
                msg,
                id,
                buf: PoolBuf::new(),
            },
            respond: None,
            cert,
            url,
        }
    }
    pub fn into_msg_respond(self) -> (wire::Wire, Option<KitsuneMockRespond>) {
        let Self { msg, respond, .. } = self;
        let KitsuneMockMsg { id, buf, msg } = msg;
        let respond = respond.map(|respond| KitsuneMockRespond { respond, id, buf });
        (msg, respond)
    }
    pub fn cert(&self) -> &Tx2Cert {
        &self.cert
    }
}

/// Create a mock network.
pub fn mock_network(
    from_kitsune_tx: FromKitsuneMockChannelTx,
    to_kitsune_rx: ToKitsuneMockChannelRx,
) -> kitsune_p2p_types::tx2::tx2_adapter::MockBindAdapt {
    let mut mock_network = kitsune_p2p_types::tx2::tx2_adapter::MockBindAdapt::new();
    mock_network.expect_bind().returning(move |_, _| {
        let from_kitsune_tx = from_kitsune_tx.clone();
        let to_kitsune_rx = to_kitsune_rx.clone();
        let resp_map = Arc::new(parking_lot::Mutex::new(HashMap::new()));
        async move {
            let mut m = MockEndpointAdapt::new();
            let uniq = Uniq::default();
            // return a uniq identifier
            m.expect_uniq().returning(move || uniq);
            // return a uniq cert
            m.expect_local_cert().returning(move || {
                // vec![TX2_CERT.fetch_add(1, std::sync::atomic::Ordering::SeqCst); 32].into()
                vec![100; 32].into()
            });
            m.expect_local_addr()
                .returning(move || Ok("http://localhost".into()));
            // allow making "outgoing" connections that will respond how
            // we configure them to
            m.expect_connect().returning({
                let from_kitsune_tx = from_kitsune_tx.clone();
                let resp_map = resp_map.clone();
                move |remote_url, _| {
                    let (resp_tx, resp_rx) = tokio::sync::mpsc::channel(1000);
                    let resp_rx = Arc::new(parking_lot::Mutex::new(Some(resp_rx)));
                    let from_kitsune_tx = from_kitsune_tx.clone();
                    let resp_rx = resp_rx.clone();
                    let resp_tx = resp_tx.clone();
                    let resp_map = resp_map.clone();
                    let cert = Tx2Cert::from(
                        kitsune_p2p_proxy::ProxyUrl::from_full(remote_url.as_str())
                            .unwrap()
                            .digest(),
                    );
                    async move {
                        // mock out our connection adapter
                        let mut m = MockConAdapt::new();
                        let uniq = Uniq::default();
                        // return a uniq identifier
                        m.expect_uniq().returning(move || uniq);
                        // this is an "outgoing" connection
                        m.expect_dir().returning(|| Tx2ConDir::Outgoing);
                        // return a uniq cert to identify our peer
                        let c = cert.clone();
                        m.expect_peer_cert().returning(move || c.clone());
                        m.expect_close()
                            .returning(move |_, _| async move { () }.boxed());
                        // allow making "outgoing" channels that will respond
                        // how we configure them to
                        m.expect_out_chan().returning(move |_| {
                            // let w_send = w_send.clone();
                            let mut m = MockAsFramedWriter::new();
                            // when we get an outgoing write event
                            // turn around and respond appropriately for
                            // our test
                            let from_kitsune_tx = from_kitsune_tx.clone();
                            let resp_tx = resp_tx.clone();
                            let resp_map = resp_map.clone();
                            let cert = cert.clone();
                            let remote_url = remote_url.clone();
                            m.expect_write().returning(move |msg_id, buf, _| {
                                // buf.cheap_move_start(65);
                                let f = write_msg(
                                    msg_id,
                                    cert.clone(),
                                    remote_url.clone(),
                                    buf,
                                    from_kitsune_tx.clone(),
                                    resp_tx.clone(),
                                    resp_map.clone(),
                                );
                                async move {
                                    f.await;
                                    Ok(())
                                }
                                .boxed()
                            });
                            let out: OutChan = Box::new(m);
                            async move { Ok(out) }.boxed()
                        });
                        let con: Arc<dyn ConAdapt> = Arc::new(m);

                        // make an incoming reader that will forward responses
                        // according to the logic in the writer above
                        let mut m = MockAsFramedReader::new();
                        m.expect_read().returning(move |_| {
                            let resp_rx = resp_rx.clone();
                            async move {
                                let mut resp = match resp_rx.lock().take() {
                                    Some(resp) => resp,
                                    None => return Err("end".into()),
                                };

                                let r = match resp.recv().await {
                                    Some(r) => match r.await {
                                        Ok(r) => {
                                            let KitsuneMockMsg { msg, id, mut buf } = r;

                                            use kitsune_p2p_types::codec::Codec;
                                            let data = msg.encode_vec().unwrap();
                                            buf.extend_from_slice(&data);
                                            (id, buf)
                                        }
                                        Err(_) => {
                                            *resp_rx.lock() = Some(resp);
                                            return Err("end".into());
                                        }
                                    },
                                    None => {
                                        *resp_rx.lock() = Some(resp);
                                        return Err("end".into());
                                    }
                                };

                                *resp_rx.lock() = Some(resp);
                                Ok(r)
                            }
                            .boxed()
                        });

                        // we'll only establish one single in channel
                        let once: InChan = Box::new(m);
                        let once =
                            futures::stream::once(async move { async move { Ok(once) }.boxed() });
                        // then just pend the stream
                        let s = once.chain(futures::stream::pending());
                        let rcv = gen_mock_in_chan_recv_adapt(s.boxed());
                        Ok((con, rcv))
                    }
                    .boxed()
                }
            });
            let ep: Arc<dyn EndpointAdapt> = Arc::new(m);

            let from_kitsune_tx = from_kitsune_tx.clone();
            let incoming = futures::stream::unfold(HashMap::new(), {
                let to_kitsune_rx = to_kitsune_rx.clone();
                move |mut open_cons: HashMap<Tx2Cert, ToKitsuneMockChannelTx>| {
                    let to_kitsune_rx = to_kitsune_rx.clone();
                    let resp_map = resp_map.clone();
                    let from_kitsune_tx = from_kitsune_tx.clone();
                    async move {
                        let mut in_rx = match to_kitsune_rx.lock().take() {
                            Some(rx) => rx,
                            None => return None,
                        };
                        while let Some(msg) = in_rx.recv().await {
                            match open_cons.get(&msg.cert) {
                                Some(tx) => {
                                    tx.send(msg).await.unwrap();
                                }
                                None => {
                                    *to_kitsune_rx.lock() = Some(in_rx);
                                    let (tx, rx) = tokio::sync::mpsc::channel(1000);
                                    let cert = msg.cert.clone();
                                    let url = msg.url.clone();
                                    tx.send(msg).await.unwrap();
                                    open_cons.insert(cert.clone(), tx);
                                    let rx = Arc::new(parking_lot::Mutex::new(Some(rx)));
                                    let out = async move {
                                        Ok(new_incoming(
                                            cert,
                                            url,
                                            rx,
                                            resp_map.clone(),
                                            from_kitsune_tx.clone(),
                                        )
                                        .await)
                                    }
                                    .boxed();
                                    return Some((out, open_cons));
                                }
                            }
                        }
                        None
                    }
                }
            })
            .boxed();
            let incoming = gen_mock_con_recv_adapt(incoming);

            Ok((ep, incoming))
        }
        .boxed()
    });
    mock_network
}

async fn new_incoming(
    cert: Tx2Cert,
    url: TxUrl,
    to_kitsune_rx: ToKitsuneMockChannelRx,
    resp_map: Arc<parking_lot::Mutex<HashMap<u64, tokio::sync::oneshot::Sender<KitsuneMockMsg>>>>,
    from_kitsune_tx: FromKitsuneMockChannelTx,
) -> Con {
    let (resp_tx, resp_rx) = tokio::sync::mpsc::channel(1000);
    let resp_rx = Arc::new(parking_lot::Mutex::new(Some(resp_rx)));
    let in_chan = futures::stream::once({
        let resp_map = resp_map.clone();
        async move {
            let resp_map = resp_map.clone();
            async move {
                let mut m = MockAsFramedReader::new();
                m.expect_read().returning(move |_| {
                    let to_kitsune_rx = to_kitsune_rx.clone();
                    let resp_map = resp_map.clone();
                    let resp_rx = resp_rx.clone();
                    async move {
                        let mut rx = match to_kitsune_rx.lock().take() {
                            Some(rx) => rx,
                            None => return Err("end".into()),
                        };
                        let mut r_rx = match resp_rx.lock().take() {
                            Some(rx) => rx,
                            None => return Err("end".into()),
                        };

                        let r = {
                            let f1 = rx.recv();
                            let f2 = r_rx.recv();
                            futures::pin_mut!(f1, f2);
                            let r = match futures::future::select(f1, f2).await {
                                futures::future::Either::Left((msg, _)) => match msg {
                                    Some(r) => {
                                        let KitsuneMock {
                                            msg: KitsuneMockMsg { msg, id, mut buf },
                                            respond,
                                            ..
                                        } = r;
                                        if let Some(respond) = respond {
                                            resp_map.lock().insert(id.as_id(), respond);
                                        }

                                        use kitsune_p2p_types::codec::Codec;
                                        let data = msg.encode_vec().unwrap();
                                        buf.extend_from_slice(&data);
                                        Ok((id, buf))
                                    }
                                    None => Err("end".into()),
                                },
                                futures::future::Either::Right((r, _)) => match r {
                                    Some(r) => match r.await {
                                        Ok(r) => {
                                            let KitsuneMockMsg { msg, id, mut buf } = r;

                                            use kitsune_p2p_types::codec::Codec;
                                            let data = msg.encode_vec().unwrap();
                                            buf.extend_from_slice(&data);
                                            Ok((id, buf))
                                        }
                                        Err(_) => Err("end".into()),
                                    },
                                    None => Err("end".into()),
                                },
                            };
                            r
                        };
                        *to_kitsune_rx.lock() = Some(rx);
                        *resp_rx.lock() = Some(r_rx);
                        r
                    }
                    .boxed()
                });
                let m: InChan = Box::new(m);
                Ok(m)
            }
            .boxed()
        }
    })
    .chain(futures::stream::pending())
    .boxed();
    let in_chan = gen_mock_in_chan_recv_adapt(in_chan);
    let mut m = MockConAdapt::new();
    let c = cert.clone();
    m.expect_peer_cert().returning(move || c.clone());
    m.expect_dir().returning(|| Tx2ConDir::Incoming);
    m.expect_uniq().returning(move || Uniq::default());
    let u = url.clone();
    m.expect_peer_addr().returning(move || Ok(u.clone()));
    m.expect_out_chan().returning({
        let resp_map = resp_map.clone();
        move |_| {
            let mut m = MockAsFramedWriter::new();
            let resp_map = resp_map.clone();
            let from_kitsune_tx = from_kitsune_tx.clone();
            let resp_tx = resp_tx.clone();
            let cert = cert.clone();
            let url = url.clone();
            m.expect_write().returning(move |msg_id, buf, _| {
                let f = write_msg(
                    msg_id,
                    cert.clone(),
                    url.clone(),
                    buf,
                    from_kitsune_tx.clone(),
                    resp_tx.clone(),
                    resp_map.clone(),
                );

                async move {
                    f.await;
                    Ok(())
                }
                .boxed()
            });
            let out: OutChan = Box::new(m);
            async move { Ok(out) }.boxed()
        }
    });
    let con: Arc<dyn ConAdapt> = Arc::new(m);
    (con, in_chan)
}

async fn write_msg(
    msg_id: MsgId,
    cert: Tx2Cert,
    url: TxUrl,
    mut buf: PoolBuf,
    from_kitsune_tx: FromKitsuneMockChannelTx,
    resp_tx: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Receiver<KitsuneMockMsg>>,
    resp_map: Arc<parking_lot::Mutex<HashMap<u64, tokio::sync::oneshot::Sender<KitsuneMockMsg>>>>,
) {
    use kitsune_p2p_types::codec::Codec;
    let respond = if msg_id.is_req() {
        let (respond, r_rx) = tokio::sync::oneshot::channel();
        resp_tx.send(r_rx).await.unwrap();
        Some(respond)
    } else {
        None
    };
    let wire = wire::Wire::decode_ref(&buf).unwrap().1;
    buf.clear();
    let msg = KitsuneMockMsg {
        msg: wire,
        id: msg_id,
        buf,
    };
    if msg_id.is_req() || msg_id.is_notify() {
        from_kitsune_tx
            .send(KitsuneMock {
                msg,
                respond,
                cert,
                url,
            })
            .await
            .unwrap();
    } else if msg_id.is_res() {
        if let Some(respond) = resp_map.lock().remove(&msg_id.as_id()) {
            respond.send(msg).unwrap();
        }
    }
}
