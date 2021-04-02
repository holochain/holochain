#![allow(clippy::new_ret_no_self)]
#![allow(clippy::blocks_in_if_conditions)]
//! Next-gen performance kitsune transport proxy

use crate::*;
use futures::future::BoxFuture;
use futures::stream::{Stream, StreamExt};
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::dependencies::serde_json;
use kitsune_p2p_types::tx2::tx2_backend::*;
use kitsune_p2p_types::tx2::tx2_pool::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::HashMap;

/// Wrap a tx2 backend transport with proxy logic.
pub fn tx2_proxy(sub_fact: EpFactory, tls_config: TlsConfig) -> EpFactory {
    ProxyEpFactory::new(sub_fact, tls_config)
}

// -- private -- //

const PROXY_TYPE_BYTES: usize = 1;
const DIGEST_BYTES: usize = 32;

//const PROXY_HELLO_DIGEST: u8 = 0x20;
const PROXY_FWD_MSG: u8 = 0x30;

struct ProxyConHnd {
    uniq: Uniq,
    sub_con: ConHnd,
    local_digest: CertDigest,
    remote_digest: CertDigest,
}

impl std::fmt::Debug for ProxyConHnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ConHnd").field(&self.uniq).finish()
    }
}

impl ProxyConHnd {
    pub fn new(sub_con: ConHnd, local_digest: CertDigest, remote_digest: CertDigest) -> ConHnd {
        let uniq = Uniq::default();
        let con = Self {
            uniq,
            sub_con,
            local_digest,
            remote_digest,
        };
        let con: ConHnd = Arc::new(con);
        con
    }
}

impl AsConHnd for ProxyConHnd {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn is_closed(&self) -> bool {
        self.sub_con.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        // TODO - FIXME
        // we don't want to close the underlying sub_con,
        // it could be shared for proxying...
        // do we want to do *anything*?
        async move {}.boxed()
    }

    fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        let peer_addr = self.sub_con.peer_addr()?;
        promote_addr(&peer_addr, &self.remote_digest)
    }

    fn peer_digest(&self) -> KitsuneResult<CertDigest> {
        Ok(self.remote_digest.clone())
    }

    fn write(
        &self,
        msg_id: MsgId,
        mut data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        data.reserve_front(PROXY_TYPE_BYTES + DIGEST_BYTES + DIGEST_BYTES);
        data.prepend_from_slice(&self.local_digest);
        data.prepend_from_slice(&self.remote_digest);
        data.prepend_from_slice(&[PROXY_FWD_MSG]);
        self.sub_con.write(msg_id, data, timeout).boxed()
    }
}

fn promote_addr(base_addr: &TxUrl, cert_digest: &CertDigest) -> KitsuneResult<TxUrl> {
    Ok(ProxyUrl::new(base_addr.as_str(), cert_digest.clone())
        .map_err(KitsuneError::other)?
        .as_str()
        .into())
}

#[allow(dead_code)]
struct ProxyEpInner {
    /*
    // this is the source of truth
    base_url_to_sub_con: HashMap<TxUrl, ConRef>,

    // allows us to remove the ConRef when a ConHnd close event is received
    sub_con_to_base_url: HashMap<ConHnd, TxUrl>,
    */

    // we only proxy over *incoming* connections
    // therefore it is a 1-to-1 relationship to remote digest
    in_digest_to_sub_con: HashMap<CertDigest, ConHnd>,

    // allows us to cleanup the digest to sub_con proxy mapping
    // when a ConHnd close event is received
    in_base_url_to_digest: HashMap<TxUrl, CertDigest>,
}

struct ProxyEpHnd {
    sub_ep_hnd: EpHnd,
    local_digest: CertDigest,
    #[allow(dead_code)]
    logic_hnd: LogicChanHandle<EpEvent>,
    inner: Share<ProxyEpInner>,
}

impl ProxyEpHnd {
    pub fn new(
        sub_ep_hnd: EpHnd,
        local_digest: CertDigest,
        logic_hnd: LogicChanHandle<EpEvent>,
    ) -> Arc<ProxyEpHnd> {
        Arc::new(ProxyEpHnd {
            sub_ep_hnd,
            local_digest,
            logic_hnd,
            inner: Share::new(ProxyEpInner {
                /*
                base_url_to_sub_con: HashMap::new(),
                sub_con_to_base_url: HashMap::new(),
                */
                in_digest_to_sub_con: HashMap::new(),
                in_base_url_to_digest: HashMap::new(),
            }),
        })
    }
}

/*
async fn ingest_outgoing_con(
    _logic_hnd: LogicChanHandle<EpEvent>,
    inner: Share<ProxyEpInner>,
    local_digest: CertDigest,
    base_url: TxUrl,
    sub_con: ConHnd,
    timeout: KitsuneTimeout,
) -> KitsuneResult<ConHnd> {
    let mut data = PoolBuf::new();
    data.reserve(local_digest.len() + 1);
    data.extend_from_slice(&[PROXY_HELLO_DIGEST]);
    data.extend_from_slice(&local_digest);
    sub_con.write(0.into(), data, timeout).await?;

    let sub_con2 = sub_con.clone();
    inner.share_mut(move |i, _| {
        i.base_url_to_sub_con
            .insert(base_url.clone(), ConRef::Ready(sub_con2.clone()));
        i.sub_con_to_base_url.insert(sub_con2, base_url);
        Ok(())
    })?;
    Ok(sub_con)
}

async fn clear_outgoing_con(
    inner: Share<ProxyEpInner>,
    base_url: TxUrl,
    err: KitsuneError,
) -> KitsuneError {
    let _ = inner.share_mut(move |i, _| {
        i.base_url_to_sub_con.remove(&base_url);
        Ok(())
    });
    err
}

async fn ingest_incoming_con(
    logic_hnd: LogicChanHandle<EpEvent>,
    inner: Share<ProxyEpInner>,
    local_digest: CertDigest,
    remote_digest: CertDigest,
    base_url: TxUrl,
    sub_con: ConHnd,
) {
    let sub_con2 = sub_con.clone();
    let remote_digest2 = remote_digest.clone();
    let _ = inner.share_mut(move |i, _| {
        i.base_url_to_sub_con
            .insert(base_url.clone(), ConRef::Ready(sub_con2.clone()));
        i.sub_con_to_base_url.insert(sub_con2.clone(), base_url);
        i.in_digest_to_sub_con
            .insert(remote_digest2.clone(), sub_con2.clone());
        i.in_sub_con_to_digest.insert(sub_con2, remote_digest2);
        Ok(())
    });
    let out_con = ProxyConHnd::new(sub_con, local_digest, remote_digest);
    let url = match out_con.remote_addr() {
        Err(e) => {
            let _ = logic_hnd.emit(EpEvent::Error(e)).await;
            return;
        }
        Ok(url) => url,
    };
    let evt = EpEvent::IncomingConnection(EpIncomingConnection { con: out_con, url });
    let _ = logic_hnd.emit(evt).await;
}
*/

impl AsEpHnd for ProxyEpHnd {
    fn debug(&self) -> serde_json::Value {
        let addr = self.local_addr();
        match self.inner.share_mut(|_i, _| {
            Ok(serde_json::json!({
                "type": "tx2_proxy",
                "state": "open",
                "addr": addr?,
                //"proxy_count": i.in_digest_to_sub_con.len(),
                "sub": self.sub_ep_hnd.debug(),
            }))
        }) {
            Ok(j) => j,
            Err(_) => serde_json::json!({
                "type": "tx2_proxy",
                "state": "closed",
            }),
        }
    }

    fn uniq(&self) -> Uniq {
        self.sub_ep_hnd.uniq()
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let local_addr = self.sub_ep_hnd.local_addr()?;
        let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), self.local_digest.clone())
            .map_err(KitsuneError::other)?
            .as_str()
            .into();
        Ok(proxy_addr)
    }

    fn is_closed(&self) -> bool {
        self.sub_ep_hnd.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        self.sub_ep_hnd.close(code, reason).boxed()
    }

    fn close_connection(&self, _remote: TxUrl, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        // TODO - FIXME
        // we don't want to close the underlying sub_con,
        // it could be shared for proxying...
        // do we want to do *anything*?
        async move {}.boxed()
    }

    fn get_connection(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let purl = ProxyUrl::from(remote.as_str());
        let remote_digest = purl.digest();
        let base_url: TxUrl = purl.as_base().as_str().into();

        let local_digest = self.local_digest.clone();
        let con_fut = self.sub_ep_hnd.get_connection(base_url, timeout);
        async move {
            let sub_con = con_fut.await?;
            Ok(ProxyConHnd::new(sub_con, local_digest, remote_digest))
        }
        .boxed()
    }
}

/*
async fn incoming_evt_logic(
    sub_ep: Ep,
    hnd: Arc<ProxyEpHnd>,
    local_digest: CertDigest,
    logic_hnd: LogicChanHandle<EpEvent>,
) {
    // erm - this const is not pub in types
    const CHANNEL_COUNT: usize = 3;

    // use CHANNEL_COUNT concurrents because that is how many channels
    // we have for sending outgoing data... most everything else in here is sync
    // and so will be processed serially anyways.
    // Benchmarks showed a slight slowdown when using semaphore count tasks
    // instead of for_each_concurrent... but maybe other problems caused that?
    sub_ep
        .for_each_concurrent(CHANNEL_COUNT, |evt| async {
            incoming_evt_handle(evt, &hnd, &local_digest, &logic_hnd).await;
        })
        .await;
}

async fn incoming_evt_handle(
    evt: EpEvent,
    hnd: &Arc<ProxyEpHnd>,
    local_digest: &CertDigest,
    logic_hnd: &LogicChanHandle<EpEvent>,
) {
    use EpEvent::*;
    match evt {
        IncomingConnection(_) => (),
        IncomingData(EpIncomingData {
            url: base_url,
            msg_id,
            mut data,
        }) => {
            if data.is_empty() {
                tracing::error!("Invalid EMPTY PROXY FRAME!");
                return;
            }
            match data[0] {
                PROXY_HELLO_DIGEST => {
                    let remote_digest = CertDigest(Arc::new(data[1..].to_vec()));
                    ingest_incoming_con(
                        logic_hnd.clone(),
                        hnd.inner.clone(),
                        local_digest.clone(),
                        remote_digest,
                        base_url,
                        sub_con,
                    )
                    .await;
                }
                PROXY_FWD_MSG => {
                    const SRC_START: usize = PROXY_TYPE_BYTES + DIGEST_BYTES;
                    const SRC_END: usize = SRC_START + DIGEST_BYTES;

                    const DEST_START: usize = PROXY_TYPE_BYTES;
                    const DEST_END: usize = DEST_START + DIGEST_BYTES;
                    let src_digest = CertDigest(Arc::new(data[SRC_START..SRC_END].to_vec()));
                    let dest_digest = CertDigest(Arc::new(data[DEST_START..DEST_END].to_vec()));
                    if dest_digest == hnd.local_digest {
                        // this data is destined for US!
                        data.cheap_move_start(SRC_END);
                        let url = promote_addr(&base_url, &src_digest).unwrap();
                        let evt = EpEvent::IncomingData(EpIncomingData {
                            url,
                            msg_id,
                            data,
                        });
                        let _ = logic_hnd.emit(evt).await;
                    } else {
                        let dest = hnd.inner.share_mut(|i, _| {
                            Ok(i.in_digest_to_sub_con.get(&dest_digest).cloned())
                        });
                        if let Err(e) = match dest {
                            Ok(Some(d_sub_con)) => {
                                let t = KitsuneTimeout::from_millis(1000 * 30);
                                d_sub_con.write(msg_id, data, t).await
                            }
                            Ok(None) => {
                                Err(format!("Invalid Proxy Target: {:?}", dest_digest).into())
                            }
                            Err(e) => Err(e),
                        } {
                            // TODO - FIXME - also respond to requestor with
                            //                an error type.
                            tracing::error!("Proxy Fwd Error: {:?}", e);
                        }
                    }
                }
                b => {
                    let reason = format!("Invalid Proxy Byte: {}", b);
                    sub_con.close(500, &reason).await;
                }
            }
        }
        ConnectionClosed(EpConnectionClosed {
            url,
            code,
            reason,
        }) => {
            let con2 = con.clone();
            let mut remote_digest = None;
            let _ = hnd.inner.share_mut(|i, _| {
                if let Some(base_url) = i.sub_con_to_base_url.remove(&con2) {
                    i.base_url_to_sub_con.remove(&base_url);
                }
                if let Some(digest) = i.in_sub_con_to_digest.remove(&con2) {
                    i.in_digest_to_sub_con.remove(&digest);
                    remote_digest = Some(digest);
                }
                Ok(())
            });
            // TODO - FIXME - better way to do this if we don't know??
            let remote_digest = remote_digest.unwrap_or_else(|| vec![0; 32].into());
            match promote_addr(&url, &remote_digest) {
                Err(e) => {
                    let _ = logic_hnd.emit(Error(e)).await;
                }
                Ok(url) => {
                    let evt = ConnectionClosed(EpConnectionClosed {
                        url,
                        code,
                        reason,
                    });
                    let _ = logic_hnd.emit(evt).await;
                }
            }
        }
        Error(e) => {
            let _ = logic_hnd.emit(Error(e)).await;
        }
        EndpointClosed => {
            let _ = hnd.inner.share_mut(|_, c| {
                *c = true;
                Ok(())
            });
            let _ = logic_hnd.emit(EndpointClosed).await;
            logic_hnd.close();
        }
    }
}
*/

async fn incoming_evt_logic(
    sub_ep: Ep,
    hnd: Arc<ProxyEpHnd>,
    local_digest: CertDigest,
    logic_hnd: LogicChanHandle<EpEvent>,
) {
    // erm - this const is not pub in types
    const CHANNEL_COUNT: usize = 3;

    // use CHANNEL_COUNT concurrents because that is how many channels
    // we have for sending outgoing data... most everything else in here is sync
    // and so will be processed serially anyways.
    // Benchmarks showed a slight slowdown when using semaphore count tasks
    // instead of for_each_concurrent... but maybe other problems caused that?
    sub_ep
        .for_each_concurrent(CHANNEL_COUNT, |evt| async {
            incoming_evt_handle(evt, &hnd, &local_digest, &logic_hnd).await;
        })
        .await;
}

async fn incoming_evt_handle(
    evt: EpEvent,
    hnd: &Arc<ProxyEpHnd>,
    _local_digest: &CertDigest,
    logic_hnd: &LogicChanHandle<EpEvent>,
) {
    println!("EVT: {:?}", evt);
    use EpEvent::*;
    match evt {
        OutgoingConnection(_) => (),
        IncomingConnection(EpConnection {
            con: sub_con,
            url: base_url,
        }) => {
            let digest = match sub_con.peer_digest() {
                Err(e) => {
                    sub_con.close(500, &format!("{:?}", e)).await;
                    return;
                }
                Ok(d) => d,
            };
            let _ = hnd.inner.share_mut(move |i, _| {
                i.in_digest_to_sub_con.insert(digest.clone(), sub_con);
                i.in_base_url_to_digest.insert(base_url, digest);
                Ok(())
            });
        }
        IncomingData(EpIncomingData {
            con: sub_con,
            url: base_url,
            msg_id,
            mut data,
        }) => {
            if data.is_empty() {
                tracing::error!("Invalid EMPTY PROXY FRAME!");
                return;
            }
            match data[0] {
                PROXY_FWD_MSG => {
                    const SRC_START: usize = PROXY_TYPE_BYTES + DIGEST_BYTES;
                    const SRC_END: usize = SRC_START + DIGEST_BYTES;

                    const DEST_START: usize = PROXY_TYPE_BYTES;
                    const DEST_END: usize = DEST_START + DIGEST_BYTES;
                    let src_digest = CertDigest(Arc::new(data[SRC_START..SRC_END].to_vec()));
                    let dest_digest = CertDigest(Arc::new(data[DEST_START..DEST_END].to_vec()));
                    if dest_digest == hnd.local_digest {
                        // this data is destined for US!
                        data.cheap_move_start(SRC_END);
                        println!("got data for US: {}", String::from_utf8_lossy(data.as_ref()));
                        let url = promote_addr(&base_url, &src_digest).unwrap();
                        let con = ProxyConHnd::new(sub_con, dest_digest, src_digest);
                        let evt = EpEvent::IncomingData(EpIncomingData {
                            con,
                            url,
                            msg_id,
                            data,
                        });
                        let _ = logic_hnd.emit(evt).await;
                    } else {
                        println!("data to forward");
                        let dest = hnd.inner.share_mut(|i, _| {
                            Ok(i.in_digest_to_sub_con.get(&dest_digest).cloned())
                        });
                        if let Err(e) = match dest {
                            Ok(Some(d_sub_con)) => {
                                let t = KitsuneTimeout::from_millis(1000 * 30);
                                d_sub_con.write(msg_id, data, t).await
                            }
                            Ok(None) => {
                                Err(format!("Invalid Proxy Target: {:?}", dest_digest).into())
                            }
                            Err(e) => Err(e),
                        } {
                            // TODO - FIXME - also respond to requestor with
                            //                an error type.
                            tracing::error!("Proxy Fwd Error: {:?}", e);
                        }
                    }
                }
                b => {
                    let reason = format!("Invalid Proxy Byte: {}", b);
                    hnd.sub_ep_hnd.close_connection(base_url, 500, &reason).await;
                }
            }
        }
        ConnectionClosed(EpConnectionClosed {
            url: base_url,
            ..
        }) => {
            let _ = hnd.inner.share_mut(|i, _| {
                if let Some(digest) = i.in_base_url_to_digest.remove(&base_url) {
                    i.in_digest_to_sub_con.remove(&digest);
                }
                Ok(())
            });

            // TODO - FIXME
            // iterate all pseudo-connections somehow
            // there isn't just one event, but all that came through the proxy
            /*
            let evt = ConnectionClosed(EpConnectionClosed {
                url,
                code,
                reason,
            });
            let _ = logic_hnd.emit(evt).await;
            */
        }
        Error(e) => {
            let _ = logic_hnd.emit(Error(e)).await;
        }
        EndpointClosed => {
            let _ = hnd.inner.share_mut(|_, c| {
                *c = true;
                Ok(())
            });
            let _ = logic_hnd.emit(EndpointClosed).await;
            logic_hnd.close();
        }
    }
}

struct ProxyEp {
    logic_chan: LogicChan<EpEvent>,
    hnd: EpHnd,
}

impl ProxyEp {
    pub async fn new(sub_ep: Ep, local_digest: CertDigest) -> KitsuneResult<Ep> {
        let logic_chan = LogicChan::new(32);
        let logic_hnd = logic_chan.handle().clone();

        let hnd = ProxyEpHnd::new(
            sub_ep.handle().clone(),
            local_digest.clone(),
            logic_hnd.clone(),
        );

        let logic = incoming_evt_logic(sub_ep, hnd.clone(), local_digest, logic_hnd);

        let l_hnd = logic_chan.handle().clone();
        l_hnd.capture_logic(logic).await?;

        let ep: Ep = Box::new(ProxyEp {
            logic_chan,
            hnd,
        });
        Ok(ep)
    }
}

impl Stream for ProxyEp {
    type Item = EpEvent;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let chan = &mut self.logic_chan;
        futures::pin_mut!(chan);
        Stream::poll_next(chan, cx)
    }
}

impl AsEp for ProxyEp {
    fn handle(&self) -> &EpHnd {
        &self.hnd
    }
}

struct ProxyEpFactory {
    tls_config: TlsConfig,
    sub_fact: EpFactory,
}

impl ProxyEpFactory {
    pub fn new(sub_fact: EpFactory, tls_config: TlsConfig) -> EpFactory {
        let fact: EpFactory = Arc::new(ProxyEpFactory {
            tls_config,
            sub_fact,
        });
        fact
    }
}

impl AsEpFactory for ProxyEpFactory {
    fn bind(
        &self,
        bind_spec: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<Ep>> {
        let digest = self.tls_config.cert_digest.clone();
        let fut = self.sub_fact.bind(bind_spec, timeout);
        async move {
            let sub_ep = fut.await?;
            ProxyEp::new(sub_ep, digest).await
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitsune_p2p_types::tx2::tx2_pool_promote::*;

    async fn build_node(
        mut s_done: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> (tokio::task::JoinHandle<()>, TxUrl, EpHnd) {
        let t = KitsuneTimeout::from_millis(5000);

        let f = tx2_pool_promote(MemBackendAdapt::new(), 32);

        let f = tx2_proxy(f, TlsConfig::new_ephemeral().await.unwrap());

        let mut ep = f.bind("none:".into(), t).await.unwrap();
        let ephnd = ep.handle().clone();
        let addr = ephnd.local_addr().unwrap();

        let join = tokio::task::spawn(async move {
            while let Some(evt) = ep.next().await {
                if let EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) = evt {
                    if data.as_ref() == b"" {
                        // pass - this is the proxy hello
                    } else if data.as_ref() == b"hello" {
                        data.clear();
                        data.extend_from_slice(b"world");
                        con.write(0.into(), data, t).await.unwrap();
                    } else if data.as_ref() == b"world" {
                        if let Some(s_done) = s_done.take() {
                            let _ = s_done.send(());
                            return;
                        }
                    } else {
                        panic!("unexpected: {}", String::from_utf8_lossy(&data));
                    }
                }
            }
        });

        (join, addr, ephnd)
    }

    fn proxify_addr(purl: &TxUrl, nurl: &TxUrl) -> TxUrl {
        let digest = ProxyUrl::from(nurl.as_str());
        let digest = digest.digest();
        let purl = ProxyUrl::from(purl.as_str());
        ProxyUrl::new(purl.as_base().as_str(), digest)
            .unwrap()
            .as_str()
            .into()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_proxy() {
        let t = KitsuneTimeout::from_millis(5000);

        let mut all_tasks = Vec::new();

        let (p_join, p_addr, p_ep) = build_node(None).await;
        all_tasks.push(p_join);
        //println!("PROXY ADDR = {}", p_addr);

        let (t_join, t_addr, t_ep) = build_node(None).await;
        all_tasks.push(t_join);

        //println!("TGT ADDR = {}", t_addr);

        // establish proxy connection
        let _ = t_ep.get_connection(p_addr.clone(), t).await.unwrap();

        let t_addr_proxy = proxify_addr(&p_addr, &t_addr);
        //println!("TGT PROXY ADDR = {}", t_addr_proxy);

        const COUNT: usize = 100;

        let mut all_futs = Vec::new();
        for _ in 0..COUNT {
            let (s_done, r_done) = tokio::sync::oneshot::channel();
            let (n_join, _n_addr, n_ep) = build_node(Some(s_done)).await;
            let t_addr_proxy = t_addr_proxy.clone();
            all_futs.push(async move {
                let mut data = PoolBuf::new();
                data.extend_from_slice(b"hello");
                n_ep.write(t_addr_proxy, 0.into(), data, t).await.unwrap();
                r_done.await.unwrap();
                n_ep.close(0, "").await;
                n_join.await.unwrap();
            });
        }

        futures::future::join_all(all_futs).await;

        let debug = p_ep.debug();
        println!("{}", serde_json::to_string_pretty(&debug).unwrap());

        p_ep.close(0, "").await;
        t_ep.close(0, "").await;

        futures::future::try_join_all(all_tasks).await.unwrap();
    }
}
