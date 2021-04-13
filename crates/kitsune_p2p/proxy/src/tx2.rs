#![allow(clippy::new_ret_no_self)]
#![allow(clippy::blocks_in_if_conditions)]
//! Next-gen performance kitsune transport proxy

use crate::*;
use futures::future::BoxFuture;
use futures::stream::{Stream, StreamExt};
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::dependencies::serde_json;
use kitsune_p2p_types::tx2::tx2_adapter::*;
use kitsune_p2p_types::tx2::tx2_pool::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::collections::HashMap;

/// Wrap a tx2 transport pool adapter with proxy logic.
pub fn tx2_proxy(sub_fact: EpFactory, tuning_params: KitsuneP2pTuningParams) -> EpFactory {
    ProxyEpFactory::new(sub_fact, tuning_params)
}

// -- private -- //

const PROXY_TYPE_BYTES: usize = 1;
const DIGEST_BYTES: usize = 32;

const PROXY_FWD_MSG: u8 = 0x30;

struct ProxyConHnd {
    uniq: Uniq,
    sub_con: ConHnd,
    local_cert: Tx2Cert,
    remote_cert: Tx2Cert,
}

impl std::fmt::Debug for ProxyConHnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ConHnd").field(&self.uniq).finish()
    }
}

impl ProxyConHnd {
    pub fn new(sub_con: ConHnd, local_cert: Tx2Cert, remote_cert: Tx2Cert) -> ConHnd {
        let uniq = Uniq::default();
        let con = Self {
            uniq,
            sub_con,
            local_cert,
            remote_cert,
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
        promote_addr(&peer_addr, &self.remote_cert)
    }

    fn peer_cert(&self) -> KitsuneResult<Tx2Cert> {
        Ok(self.remote_cert.clone())
    }

    fn write(
        &self,
        msg_id: MsgId,
        mut data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        data.reserve_front(PROXY_TYPE_BYTES + DIGEST_BYTES + DIGEST_BYTES);
        data.prepend_from_slice(&self.local_cert);
        data.prepend_from_slice(&self.remote_cert);
        data.prepend_from_slice(&[PROXY_FWD_MSG]);
        self.sub_con.write(msg_id, data, timeout).boxed()
    }
}

fn promote_addr(base_addr: &TxUrl, cert: &Tx2Cert) -> KitsuneResult<TxUrl> {
    Ok(ProxyUrl::new(base_addr.as_str(), cert.as_digest().clone())
        .map_err(KitsuneError::other)?
        .as_str()
        .into())
}

#[allow(dead_code)]
struct ProxyEpInner {
    // we only proxy over *incoming* connections
    // therefore it is a 1-to-1 relationship to remote digest
    in_digest_to_sub_con: HashMap<Tx2Cert, ConHnd>,

    // allows us to cleanup the digest to sub_con proxy mapping
    // when a ConHnd close event is received
    in_base_url_to_digest: HashMap<TxUrl, Tx2Cert>,

    // allows us to clone Tx2ConHnd items which will share
    // the same Uniq, rather than duplicating handles to the same connection.
    base_url_to_uniq_out_con_hnd: HashMap<TxUrl, HashMap<Tx2Cert, ConHnd>>,
}

impl ProxyEpInner {
    pub fn get_con_hnd(
        &mut self,
        sub_con: ConHnd,
        local_cert: Tx2Cert,
        remote_cert: Tx2Cert,
    ) -> KitsuneResult<(bool, ConHnd)> {
        let base_url = sub_con.peer_addr()?;
        let inner_map = self
            .base_url_to_uniq_out_con_hnd
            .entry(base_url)
            .or_insert_with(HashMap::new);
        let mut did_insert = false;
        let con = {
            let did_insert = &mut did_insert;
            inner_map
                .entry(remote_cert.clone())
                .or_insert_with(move || {
                    *did_insert = true;
                    ProxyConHnd::new(sub_con, local_cert, remote_cert)
                })
                .clone()
        };
        Ok((did_insert, con))
    }
}

struct ProxyEpHnd {
    sub_ep_hnd: EpHnd,
    local_cert: Tx2Cert,
    logic_hnd: LogicChanHandle<EpEvent>,
    inner: Share<ProxyEpInner>,
}

async fn get_con_hnd(
    inner: &Share<ProxyEpInner>,
    logic_hnd: LogicChanHandle<EpEvent>,
    sub_con: ConHnd,
    local_cert: Tx2Cert,
    remote_cert: Tx2Cert,
    is_outgoing: bool,
) -> KitsuneResult<ConHnd> {
    let (did_insert, con) =
        inner.share_mut(move |i, _| i.get_con_hnd(sub_con, local_cert, remote_cert))?;
    if did_insert {
        let con = con.clone();
        let url = con.peer_addr()?;
        let evt = if is_outgoing {
            EpEvent::OutgoingConnection(EpConnection { con, url })
        } else {
            EpEvent::IncomingConnection(EpConnection { con, url })
        };
        let _ = logic_hnd.emit(evt).await;
    }
    Ok(con)
}

impl ProxyEpHnd {
    pub fn new(
        sub_ep_hnd: EpHnd,
        logic_hnd: LogicChanHandle<EpEvent>,
    ) -> KitsuneResult<Arc<ProxyEpHnd>> {
        let local_cert = sub_ep_hnd.local_cert()?;
        Ok(Arc::new(ProxyEpHnd {
            sub_ep_hnd,
            local_cert,
            logic_hnd,
            inner: Share::new(ProxyEpInner {
                in_digest_to_sub_con: HashMap::new(),
                in_base_url_to_digest: HashMap::new(),
                base_url_to_uniq_out_con_hnd: HashMap::new(),
            }),
        }))
    }
}

impl AsEpHnd for ProxyEpHnd {
    fn debug(&self) -> serde_json::Value {
        let addr = self.local_addr();
        match self.inner.share_mut(|i, _| {
            Ok(serde_json::json!({
                "type": "tx2_proxy",
                "state": "open",
                "addr": addr?,
                "proxy_count": i.in_digest_to_sub_con.len(),
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
        let proxy_addr: TxUrl =
            ProxyUrl::new(local_addr.as_str(), self.local_cert.as_digest().clone())
                .map_err(KitsuneError::other)?
                .as_str()
                .into();
        Ok(proxy_addr)
    }

    fn local_cert(&self) -> KitsuneResult<Tx2Cert> {
        self.sub_ep_hnd.local_cert()
    }

    fn is_closed(&self) -> bool {
        self.sub_ep_hnd.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        self.sub_ep_hnd.close(code, reason).boxed()
    }

    fn close_connection(
        &self,
        _remote: TxUrl,
        _code: u32,
        _reason: &str,
    ) -> BoxFuture<'static, ()> {
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
        let remote_cert = purl.digest().into();
        if remote_cert == self.local_cert {
            tracing::warn!("refusing outgoing connection to node with same cert");
            return async move {
                Err("refusing outgoing connection to node with same cert".into())
            }.boxed();
        }

        let base_url: TxUrl = purl.as_base().as_str().into();

        let local_cert = self.local_cert.clone();
        let logic_hnd = self.logic_hnd.clone();
        let con_fut = self.sub_ep_hnd.get_connection(base_url, timeout);
        let inner = self.inner.clone();
        async move {
            let sub_con = con_fut.await?;
            get_con_hnd(&inner, logic_hnd, sub_con, local_cert, remote_cert, true).await
        }
        .boxed()
    }
}

async fn incoming_evt_logic(
    tuning_params: KitsuneP2pTuningParams,
    sub_ep: Ep,
    hnd: Arc<ProxyEpHnd>,
    logic_hnd: LogicChanHandle<EpEvent>,
) {
    let local_cert = match sub_ep.handle().local_cert() {
        Err(_) => {
            tracing::warn!("ep closed before evt handler launch");
            return;
        }
        Ok(d) => d,
    };
    let local_cert = &local_cert;

    // use CHANNEL_COUNT concurrents because that is how many channels
    // we have for sending outgoing data... most everything else in here is sync
    // and so will be processed serially anyways.
    // Benchmarks showed a slight slowdown when using semaphore count tasks
    // instead of for_each_concurrent... but maybe other problems caused that?
    sub_ep
        .for_each_concurrent(
            tuning_params.tx2_channel_count_per_connection,
            |evt| async {
                incoming_evt_handle(evt, local_cert.clone(), &hnd, &logic_hnd).await;
            },
        )
        .await;
}

async fn incoming_evt_handle(
    evt: EpEvent,
    local_cert: Tx2Cert,
    hnd: &Arc<ProxyEpHnd>,
    logic_hnd: &LogicChanHandle<EpEvent>,
) {
    //println!("EVT: {:?}", evt);
    use EpEvent::*;
    match evt {
        OutgoingConnection(_) => (),
        IncomingConnection(EpConnection {
            con: sub_con,
            url: base_url,
        }) => {
            let cert = match sub_con.peer_cert() {
                Err(e) => {
                    sub_con.close(500, &format!("{:?}", e)).await;
                    return;
                }
                Ok(d) => d,
            };
            if cert == local_cert {
                sub_con
                    .close(500, "refusing connection with matching cert")
                    .await;
                tracing::warn!("refusing connection with matching cert");
                return;
            }
            let _ = hnd.inner.share_mut(move |i, _| {
                i.in_digest_to_sub_con.insert(cert.clone(), sub_con);
                i.in_base_url_to_digest.insert(base_url, cert);
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
                    let src_cert = data[SRC_START..SRC_END].to_vec().into();
                    let dest_cert = data[DEST_START..DEST_END].to_vec().into();
                    //println!("src: {:?}", src_cert);
                    //println!("dst: {:?}", dest_cert);
                    //println!("loc: {:?}", hnd.local_cert);
                    if dest_cert == hnd.local_cert {
                        // this data is destined for US!
                        data.cheap_move_start(SRC_END);
                        //println!("got data for US: {}", String::from_utf8_lossy(data.as_ref()));
                        let url = promote_addr(&base_url, &src_cert).unwrap();
                        let con = match get_con_hnd(
                            &hnd.inner,
                            logic_hnd.clone(),
                            sub_con,
                            dest_cert,
                            src_cert,
                            false,
                        )
                        .await
                        {
                            Err(_) => return,
                            Ok(con) => con,
                        };
                        let evt = EpEvent::IncomingData(EpIncomingData {
                            con,
                            url,
                            msg_id,
                            data,
                        });
                        let _ = logic_hnd.emit(evt).await;
                    } else {
                        //println!("data to forward");
                        let dest = hnd.inner.share_mut(|i, _| {
                            //println!("ALALA: {:?}", i.in_digest_to_sub_con);
                            Ok(i.in_digest_to_sub_con.get(&dest_cert).cloned())
                        });
                        if let Err(e) = match dest {
                            Ok(Some(d_sub_con)) => {
                                let t = KitsuneTimeout::from_millis(1000 * 30);
                                d_sub_con.write(msg_id, data, t).await
                            }
                            Ok(None) => {
                                Err(format!("Invalid Proxy Target: {:?}", dest_cert).into())
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
                    hnd.sub_ep_hnd
                        .close_connection(base_url, 500, &reason)
                        .await;
                }
            }
        }
        ConnectionClosed(EpConnectionClosed {
            url: base_url,
            code,
            reason,
        }) => {
            let kill_cons = hnd.inner.share_mut(|i, _| {
                if let Some(cert) = i.in_base_url_to_digest.remove(&base_url) {
                    i.in_digest_to_sub_con.remove(&cert);
                }
                Ok(i.base_url_to_uniq_out_con_hnd.remove(&base_url))
            });

            let kill_cons = match kill_cons {
                Ok(Some(c)) => c,
                _ => return,
            };

            for (_, c) in kill_cons.into_iter() {
                let url = match c.peer_addr() {
                    Ok(url) => url,
                    _ => continue,
                };
                let evt = ConnectionClosed(EpConnectionClosed {
                    url,
                    code,
                    reason: reason.clone(),
                });
                let _ = logic_hnd.emit(evt).await;
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

struct ProxyEp {
    logic_chan: LogicChan<EpEvent>,
    hnd: EpHnd,
}

impl ProxyEp {
    pub async fn new(sub_ep: Ep, tuning_params: KitsuneP2pTuningParams) -> KitsuneResult<Ep> {
        // this isn't something that needs to be configurable,
        // because it's entirely dependent on the code written here
        // we only ever capture a singe logic closure
        // so technically, it only really would need to be 1.
        const LOGIC_CHAN_LIMIT: usize = 32;

        let logic_chan = LogicChan::new(LOGIC_CHAN_LIMIT);
        let logic_hnd = logic_chan.handle().clone();

        let hnd = ProxyEpHnd::new(sub_ep.handle().clone(), logic_hnd.clone())?;

        let logic = incoming_evt_logic(tuning_params, sub_ep, hnd.clone(), logic_hnd);

        let l_hnd = logic_chan.handle().clone();
        l_hnd.capture_logic(logic).await?;

        let ep: Ep = Box::new(ProxyEp { logic_chan, hnd });
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
    tuning_params: KitsuneP2pTuningParams,
    sub_fact: EpFactory,
}

impl ProxyEpFactory {
    pub fn new(sub_fact: EpFactory, tuning_params: KitsuneP2pTuningParams) -> EpFactory {
        let fact: EpFactory = Arc::new(ProxyEpFactory {
            tuning_params,
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
        let tuning_params = self.tuning_params.clone();
        let fut = self.sub_fact.bind(bind_spec, timeout);
        async move {
            let sub_ep = fut.await?;
            ProxyEp::new(sub_ep, tuning_params).await
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

        let f = tx2_mem_adapter(MemConfig::default()).await.unwrap();
        let f = tx2_pool_promote(f, Default::default());

        let f = tx2_proxy(f, Default::default());

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
        observability::test_run().ok();

        let t = KitsuneTimeout::from_millis(5000);

        let mut all_tasks = Vec::new();

        let (p_join, p_addr, p_ep) = build_node(None).await;
        all_tasks.push(p_join);
        //println!("PROXY ADDR = {}", p_addr);
        //println!("PROXY: {:?}", p_ep.local_cert().unwrap());

        let (t_join, t_addr, t_ep) = build_node(None).await;
        all_tasks.push(t_join);

        //println!("TGT ADDR = {}", t_addr);
        //println!("TGT: {:?}", t_ep.local_cert().unwrap());

        // establish proxy connection
        let _ = t_ep.get_connection(p_addr.clone(), t).await.unwrap();

        let t_addr_proxy = proxify_addr(&p_addr, &t_addr);
        //println!("TGT PROXY ADDR = {}", t_addr_proxy);

        const COUNT: usize = 100;

        let mut all_futs = Vec::new();
        for _ in 0..COUNT {
            let (s_done, r_done) = tokio::sync::oneshot::channel();
            let (n_join, _n_addr, n_ep) = build_node(Some(s_done)).await;
            //println!("N: {:?}", n_ep.local_cert().unwrap());

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
