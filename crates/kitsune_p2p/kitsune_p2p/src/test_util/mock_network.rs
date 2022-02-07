use std::collections::HashMap;
use std::sync::Arc;

use futures::future::Either;
use futures::FutureExt;
use futures::StreamExt;
use kitsune_p2p_types::tx2::tx2_adapter::test_utils::*;
use kitsune_p2p_types::tx2::tx2_adapter::*;
use kitsune_p2p_types::tx2::tx2_utils::PoolBuf;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::KitsuneError;
use kitsune_p2p_types::KitsuneResult;
use kitsune_p2p_types::Tx2Cert;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::wire;

pub type FromKitsuneMockChannelTx = mpsc::Sender<KitsuneMock>;
pub type FromKitsuneMockChannelRx = mpsc::Receiver<KitsuneMock>;
pub type ToKitsuneMockChannelTx = mpsc::Sender<KitsuneMock>;
pub struct ToKitsuneMockChannelRx(SharedRecv<KitsuneMock>);

#[derive(Debug)]
pub struct KitsuneMock {
    msg: KitsuneMockMsg,
    respond: Option<oneshot::Sender<KitsuneMockMsg>>,
    cert: Tx2Cert,
    url: TxUrl,
}

#[derive(Debug)]
pub struct KitsuneMockMsg {
    msg: wire::Wire,
    id: MsgId,
    buf: PoolBuf,
}

impl KitsuneMockMsg {
    /// Get the underlying wire message and discard
    /// the id / buffer.
    pub fn into_wire(self) -> wire::Wire {
        self.msg
    }
}

#[derive(Debug)]
pub struct KitsuneMockRespond {
    respond: oneshot::Sender<KitsuneMockMsg>,
    id: MsgId,
    buf: PoolBuf,
}

pub fn to_kitsune_channel(buffer: usize) -> (ToKitsuneMockChannelTx, ToKitsuneMockChannelRx) {
    let (tx, rx) = mpsc::channel(buffer);
    (tx, ToKitsuneMockChannelRx(SharedRecv::new(rx)))
}

impl KitsuneMockRespond {
    pub fn respond(self, msg: wire::Wire) {
        let Self { respond, id, buf } = self;
        let _ = respond.send(KitsuneMockMsg {
            msg,
            id: id.as_res(),
            buf,
        });
    }
}

impl KitsuneMock {
    pub fn request(
        id: MsgId,
        cert: Tx2Cert,
        url: TxUrl,
        msg: wire::Wire,
        respond: oneshot::Sender<KitsuneMockMsg>,
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

struct MockNetwork {
    from_kitsune_tx: FromKitsuneMockChannelTx,
    to_kitsune_rx: ToKitsuneMockChannelRx,
    response_map: Arc<parking_lot::Mutex<HashMap<u64, oneshot::Sender<KitsuneMockMsg>>>>,
}

struct MockOutConnection {
    response_tx: mpsc::Sender<oneshot::Receiver<KitsuneMockMsg>>,
    response_rx: SharedRecv<oneshot::Receiver<KitsuneMockMsg>>,
    remote_url: TxUrl,
    remote_cert: Tx2Cert,
}

struct MockInConnection {
    incoming_rx: SharedRecv<KitsuneMock>,
    out_connection: Arc<MockOutConnection>,
}

/// A type to allow sharing a single receiver between threads.
/// This is not able to be used concurrently as it will panic.
/// It is needed to allow the receiver to be moved into a single future
/// at a time.
struct SharedRecv<T> {
    recv: Arc<parking_lot::Mutex<Option<mpsc::Receiver<T>>>>,
}

impl MockOutConnection {
    fn new(remote_url: TxUrl) -> Arc<Self> {
        let (response_tx, response_rx) = mpsc::channel(1000);
        let response_rx = SharedRecv::new(response_rx);
        let remote_cert = url_to_cert(&remote_url);
        Arc::new(Self {
            response_tx,
            response_rx,
            remote_url,
            remote_cert,
        })
    }
}

impl MockInConnection {
    fn new(msg: &KitsuneMock) -> (mpsc::Sender<KitsuneMock>, Arc<Self>) {
        let (response_tx, response_rx) = mpsc::channel(1000);
        let response_rx = SharedRecv::new(response_rx);
        let (incoming_tx, incoming_rx) = mpsc::channel(1000);
        let incoming_rx = SharedRecv::new(incoming_rx);
        let out_connection = MockOutConnection {
            response_tx,
            response_rx,
            remote_cert: msg.cert.clone(),
            remote_url: msg.url.clone(),
        };
        let s = Self {
            incoming_rx,
            out_connection: Arc::new(out_connection),
        };
        (incoming_tx, Arc::new(s))
    }
}

/// Create a mock network.
pub fn mock_network(
    from_kitsune_tx: FromKitsuneMockChannelTx,
    to_kitsune_rx: ToKitsuneMockChannelRx,
) -> kitsune_p2p_types::tx2::tx2_adapter::MockBindAdapt {
    let mut mock_network = kitsune_p2p_types::tx2::tx2_adapter::MockBindAdapt::new();
    let response_map = Arc::new(parking_lot::Mutex::new(HashMap::new()));
    let network = MockNetwork {
        from_kitsune_tx,
        to_kitsune_rx,
        response_map,
    };
    let network = Arc::new(network);
    mock_network
        .expect_local_cert()
        .returning(move || vec![0; 32].into());
    mock_network.expect_bind().returning(move |local_addr, _| {
        let network = network.clone();
        async move {
            let mut m = MockEndpointAdapt::new();
            let uniq = Uniq::default();
            m.expect_uniq().returning(move || uniq);
            m.expect_local_cert().returning(move || vec![0; 32].into());
            m.expect_local_addr()
                .returning(move || Ok(local_addr.clone()));
            m.expect_connect().returning({
                let network = network.clone();
                move |remote_url, _| {
                    let network = network.clone();
                    let connection = MockOutConnection::new(remote_url);
                    async move {
                        // mock out our connection adapter
                        let mut m = MockConAdapt::new();
                        // return a uniq identifier
                        let uniq = Uniq::default();
                        m.expect_uniq().returning(move || uniq);
                        // this is an "outgoing" connection
                        m.expect_dir().returning(|| Tx2ConDir::Outgoing);
                        // return a uniq cert to identify our peer
                        let cert = connection.remote_cert.clone();
                        m.expect_peer_cert().returning(move || cert.clone());
                        m.expect_close()
                            .returning(move |_, _| async move {}.boxed());
                        // allow making "outgoing" channels that will respond
                        // how we configure them to
                        m.expect_out_chan().returning({
                            let connection = connection.clone();
                            move |_| {
                                let mut m = MockAsFramedWriter::new();
                                let network = network.clone();
                                let connection = connection.clone();
                                m.expect_write().returning(move |msg_id, buf, _| {
                                    write_msg(msg_id, buf, network.clone(), connection.clone())
                                        .boxed()
                                });
                                let out: OutChan = Box::new(m);
                                async move { Ok(out) }.boxed()
                            }
                        });
                        let con: Arc<dyn ConAdapt> = Arc::new(m);

                        // make an incoming reader that will forward responses
                        // according to the logic in the writer above
                        let mut m = MockAsFramedReader::new();
                        let connection = connection.clone();
                        m.expect_read().returning(move |_| {
                            let connection = connection.clone();
                            read_response(connection).boxed()
                        });

                        // we'll only establish one single in channel
                        let once: InChan = Box::new(m);
                        #[allow(clippy::async_yields_async)]
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

            let incoming = futures::stream::unfold(HashMap::new(), {
                let network = network.clone();
                move |mut open_cons: HashMap<Tx2Cert, ToKitsuneMockChannelTx>| {
                    let network = network.clone();
                    async move {
                        while let Some(msg) = network.to_kitsune_rx.0.recv().await {
                            match open_cons.get(&msg.cert) {
                                Some(tx) => {
                                    tx.send(msg).await.unwrap();
                                }
                                None => {
                                    let (incoming_tx, incoming_connection) =
                                        MockInConnection::new(&msg);
                                    let cert = msg.cert.clone();
                                    incoming_tx.send(msg).await.unwrap();
                                    open_cons.insert(cert, incoming_tx);
                                    let out = async move {
                                        Ok(new_incoming(network.clone(), incoming_connection).await)
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

async fn new_incoming(network: Arc<MockNetwork>, connection: Arc<MockInConnection>) -> Con {
    let in_chan = futures::stream::once({
        let connection = connection.clone();
        let network = network.clone();
        #[allow(clippy::async_yields_async)]
        async move {
            let network = network.clone();
            let connection = connection.clone();
            async move {
                let network = network.clone();
                let connection = connection.clone();
                let mut m = MockAsFramedReader::new();
                m.expect_read().returning(move |_| {
                    let network = network.clone();
                    let connection = connection.clone();
                    async move {
                        let result = connection
                            .incoming_rx
                            .select_recv(connection.out_connection.response_rx.clone())
                            .await;
                        match result {
                            Some(Either::Left(r)) => {
                                let KitsuneMock {
                                    msg: KitsuneMockMsg { msg, id, mut buf },
                                    respond,
                                    ..
                                } = r;
                                if let Some(respond) = respond {
                                    network.response_map.lock().insert(id.as_id(), respond);
                                }

                                use kitsune_p2p_types::codec::Codec;
                                let data = msg.encode_vec().map_err(k_error)?;
                                buf.extend_from_slice(&data);
                                Ok((id, buf))
                            }
                            Some(Either::Right(r)) => match r.await {
                                Ok(r) => {
                                    let KitsuneMockMsg { msg, id, mut buf } = r;

                                    use kitsune_p2p_types::codec::Codec;
                                    let data = msg.encode_vec().map_err(k_error)?;
                                    buf.extend_from_slice(&data);
                                    Ok((id, buf))
                                }
                                Err(_) => Err("end".into()),
                            },
                            None => Err("end".into()),
                        }
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
    let cert = connection.out_connection.remote_cert.clone();

    m.expect_peer_cert().returning(move || cert.clone());
    m.expect_dir().returning(|| Tx2ConDir::Incoming);
    let uniq = Uniq::default();
    m.expect_uniq().returning(move || uniq);
    let url = connection.out_connection.remote_url.clone();
    m.expect_peer_addr().returning(move || Ok(url.clone()));
    m.expect_out_chan().returning({
        move |_| {
            let mut m = MockAsFramedWriter::new();
            let network = network.clone();
            let connection = connection.clone();
            m.expect_write().returning(move |msg_id, buf, _| {
                write_msg(
                    msg_id,
                    buf,
                    network.clone(),
                    connection.out_connection.clone(),
                )
                .boxed()
            });
            let out: OutChan = Box::new(m);
            async move { Ok(out) }.boxed()
        }
    });
    m.expect_close()
        .returning(move |_, _| async move {}.boxed());
    let con: Arc<dyn ConAdapt> = Arc::new(m);
    (con, in_chan)
}

async fn write_msg(
    msg_id: MsgId,
    mut buf: PoolBuf,
    network: Arc<MockNetwork>,
    connection: Arc<MockOutConnection>,
) -> KitsuneResult<()> {
    use kitsune_p2p_types::codec::Codec;
    let respond = if msg_id.is_req() {
        let (respond, r_rx) = oneshot::channel();
        connection.response_tx.send(r_rx).await.map_err(k_error)?;
        Some(respond)
    } else {
        None
    };
    let wire = wire::Wire::decode_ref(&buf).map_err(k_error)?.1;
    buf.clear();
    let msg = KitsuneMockMsg {
        msg: wire,
        id: msg_id,
        buf,
    };
    if msg_id.is_req() || msg_id.is_notify() {
        network
            .from_kitsune_tx
            .send(KitsuneMock {
                msg,
                respond,
                cert: connection.remote_cert.clone(),
                url: connection.remote_url.clone(),
            })
            .await
            .map_err(k_error)?;
    } else if msg_id.is_res() {
        if let Some(respond) = network.response_map.lock().remove(&msg_id.as_id()) {
            respond.send(msg).map_err(k_error)?;
        }
    }
    Ok(())
}

fn k_error<E: std::fmt::Debug>(e: E) -> KitsuneError {
    format!("{:?}", e).into()
}

fn url_to_cert(url: &TxUrl) -> Tx2Cert {
    Tx2Cert::from(
        kitsune_p2p_proxy::ProxyUrl::from_full(url.as_str())
            .expect("Mock network failed to parse url")
            .digest(),
    )
}

async fn read_response(connection: Arc<MockOutConnection>) -> KitsuneResult<(MsgId, PoolBuf)> {
    let r = match connection.response_rx.recv().await {
        Some(r) => match r.await {
            Ok(r) => {
                let KitsuneMockMsg { msg, id, mut buf } = r;

                use kitsune_p2p_types::codec::Codec;
                let data = msg.encode_vec().map_err(k_error)?;
                buf.extend_from_slice(&data);
                (id, buf)
            }
            Err(_) => {
                return Err("end".into());
            }
        },
        None => {
            return Err("end".into());
        }
    };

    Ok(r)
}

impl<T> SharedRecv<T> {
    fn new(recv: mpsc::Receiver<T>) -> Self {
        Self {
            recv: Arc::new(parking_lot::Mutex::new(Some(recv))),
        }
    }

    async fn recv(&self) -> Option<T> {
        let mut recv = {
            self.recv
                .lock()
                .take()
                .expect("This type cannot be used concurrently")
        };
        let r = recv.recv().await;
        *self.recv.lock() = Some(recv);
        r
    }

    async fn select_recv<U>(&self, other: SharedRecv<U>) -> Option<Either<T, U>> {
        let mut a = {
            self.recv
                .lock()
                .take()
                .expect("This type cannot be used concurrently")
        };
        let mut b = {
            other
                .recv
                .lock()
                .take()
                .expect("This type cannot be used concurrently")
        };
        let r = {
            let f_a = a.recv();
            let f_b = b.recv();
            futures::pin_mut!(f_a, f_b);
            match futures::future::select(f_a, f_b).await {
                Either::Left((t, _)) => t.map(Either::Left),
                Either::Right((u, _)) => u.map(Either::Right),
            }
        };

        *self.recv.lock() = Some(a);
        *other.recv.lock() = Some(b);
        r
    }
}

impl<T> Clone for SharedRecv<T> {
    fn clone(&self) -> Self {
        Self {
            recv: self.recv.clone(),
        }
    }
}
