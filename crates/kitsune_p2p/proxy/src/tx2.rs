#![allow(clippy::new_ret_no_self)]
#![allow(clippy::blocks_in_if_conditions)]
//! Next-gen performance kitsune transport proxy

use crate::*;
use futures::future::BoxFuture;
use futures::stream::{Stream, StreamExt};
use kitsune_p2p_types::tx2::tx2_backend::*;
use kitsune_p2p_types::tx2::tx2_frontend::tx2_frontend_traits::*;
use kitsune_p2p_types::tx2::tx2_frontend::*;
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

const PROXY_HELLO_DIGEST: u8 = 0x20;
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
        ConHnd(Arc::new(con))
    }
}

fn promote_addr(base_addr: &TxUrl, cert_digest: &CertDigest) -> KitsuneResult<TxUrl> {
    Ok(ProxyUrl::new(base_addr.as_str(), cert_digest.clone())
        .map_err(KitsuneError::other)?
        .as_str()
        .into())
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

    fn remote_addr(&self) -> KitsuneResult<TxUrl> {
        let remote_addr = self.sub_con.remote_addr()?;
        promote_addr(&remote_addr, &self.remote_digest)
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

/// Represents either an unresolved (i.e. still pending) connection,
/// or an already established connection.
#[allow(dead_code)]
#[derive(Clone)]
enum ConRef {
    Ready(ConHnd),
    Pending(futures::future::Shared<BoxFuture<'static, KitsuneResult<ConHnd>>>),
}

#[allow(dead_code)]
struct ProxyEpInner {
    // this is the source of truth
    base_url_to_sub_con: HashMap<TxUrl, ConRef>,

    // allows us to remove the ConRef when a ConHnd close event is received
    sub_con_to_base_url: HashMap<ConHnd, TxUrl>,

    // we only proxy over *incoming* connections
    // therefore it is a 1-to-1 relationship to remote digest
    in_digest_to_sub_con: HashMap<CertDigest, ConHnd>,

    // allows us to cleanup the digest to sub_con proxy mapping
    // when a ConHnd close event is received
    in_sub_con_to_digest: HashMap<ConHnd, CertDigest>,
}

#[allow(dead_code)]
struct ProxyEpHnd {
    sub_ep_hnd: EpHnd,
    cert_digest: CertDigest,
    logic_hnd: LogicChanHandle<EpEvent>,
    inner: Share<ProxyEpInner>,
}

impl ProxyEpHnd {
    pub fn new(
        sub_ep_hnd: EpHnd,
        cert_digest: CertDigest,
        logic_hnd: LogicChanHandle<EpEvent>,
    ) -> Arc<ProxyEpHnd> {
        Arc::new(ProxyEpHnd {
            sub_ep_hnd,
            cert_digest,
            logic_hnd,
            inner: Share::new(ProxyEpInner {
                base_url_to_sub_con: HashMap::new(),
                sub_con_to_base_url: HashMap::new(),
                in_digest_to_sub_con: HashMap::new(),
                in_sub_con_to_digest: HashMap::new(),
            }),
        })
    }
}

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

impl AsEpHnd for ProxyEpHnd {
    fn uniq(&self) -> Uniq {
        self.sub_ep_hnd.uniq()
    }

    fn is_closed(&self) -> bool {
        self.sub_ep_hnd.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        self.sub_ep_hnd.close(code, reason).boxed()
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let local_addr = self.sub_ep_hnd.local_addr()?;
        let proxy_addr: TxUrl = ProxyUrl::new(local_addr.as_str(), self.cert_digest.clone())
            .map_err(KitsuneError::other)?
            .as_str()
            .into();
        Ok(proxy_addr)
    }

    fn connect(
        &self,
        remote: TxUrl,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<ConHnd>> {
        let local_digest = self.cert_digest.clone();
        let logic_hnd = self.logic_hnd.clone();
        let sub_ep_hnd = self.sub_ep_hnd.clone();
        let inner = self.inner.clone();
        async move {
            let url = ProxyUrl::from(remote.as_str());
            let remote_digest = url.digest();
            if remote_digest == local_digest {
                return Err("refusing to connect to self".into());
            }

            let inner2 = inner.clone();
            let base: TxUrl = url.as_base().as_str().into();
            let base2 = base.clone();
            let local_digest2 = local_digest.clone();
            let connect = move || {
                ConRef::Pending(
                    async move {
                        match sub_ep_hnd.connect(base2.clone(), timeout).await {
                            Ok(con) => {
                                ingest_outgoing_con(
                                    logic_hnd,
                                    inner2,
                                    local_digest2,
                                    base2,
                                    con,
                                    timeout,
                                )
                                .await
                            }
                            Err(e) => Err(clear_outgoing_con(inner2, base2, e).await),
                        }
                    }
                    .boxed()
                    .shared(),
                )
            };

            let r = inner.share_mut(|i, _| {
                Ok(i.base_url_to_sub_con
                    .entry(base)
                    .or_insert_with(connect)
                    .clone())
            })?;

            let sub_con = match r {
                ConRef::Ready(c) => c,
                ConRef::Pending(p) => p.await?,
            };

            let out_con = ProxyConHnd::new(sub_con, local_digest, remote_digest);
            Ok(out_con)
        }
        .boxed()
    }
}

async fn incoming_evt_logic(
    mut sub_ep: Ep,
    hnd: Arc<ProxyEpHnd>,
    local_digest: CertDigest,
    logic_hnd: LogicChanHandle<EpEvent>,
) {
    while let Some(evt) = sub_ep.next().await {
        use EpEvent::*;
        if match evt {
            IncomingConnection(_) => Ok(()),
            IncomingData(EpIncomingData {
                con: sub_con,
                url: base_url,
                msg_id,
                mut data,
            }) => {
                if data.is_empty() {
                    // TODO - FIXME - kill connection
                    panic!("corrupt incoming data");
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
                        if dest_digest == hnd.cert_digest {
                            // this data is destined for US!
                            data.cheap_move_start(SRC_END);
                            let url = match promote_addr(&base_url, &src_digest) {
                                Ok(url) => url,
                                // TODO - FIXME
                                e => panic!("{:?}", e),
                            };
                            let con = ProxyConHnd::new(sub_con, dest_digest, src_digest);
                            let evt = EpEvent::IncomingData(EpIncomingData {
                                con,
                                url,
                                msg_id,
                                data,
                            });
                            let _ = logic_hnd.emit(evt).await;
                        } else {
                            let dest = hnd.inner.share_mut(|i, _| {
                                Ok(i.in_digest_to_sub_con.get(&dest_digest).cloned())
                            });
                            match dest {
                                Ok(Some(d_sub_con)) => {
                                    let t = KitsuneTimeout::from_millis(1000 * 30);
                                    // TODO - correct to await here / backpressure?
                                    // TODO - respond with errors?
                                    let _ = d_sub_con.write(msg_id, data, t).await;
                                }
                                // TODO - FIXME
                                e => panic!("{:?}", e),
                            }
                        }
                    }
                    // TODO - FIXME - kill connection
                    _ => panic!("corrupt incoming data"),
                }
                Ok(())
            }
            ConnectionClosed(EpConnectionClosed {
                con,
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
                        let _ = logic_hnd.emit(Error(e));
                        Ok(())
                    }
                    Ok(url) => {
                        let evt = ConnectionClosed(EpConnectionClosed {
                            con,
                            url,
                            code,
                            reason,
                        });
                        let _ = logic_hnd.emit(evt).await;
                        Ok(())
                    }
                }
            }
            Error(e) => logic_hnd.emit(Error(e)).await,
            EndpointClosed => {
                let _ = hnd.inner.share_mut(|_, c| {
                    *c = true;
                    Ok(())
                });
                let _ = logic_hnd.emit(EndpointClosed).await;
                logic_hnd.close();
                Ok(())
            }
        }
        .is_err()
        {
            break;
        }
    }
}

struct ProxyEp {
    logic_chan: LogicChan<EpEvent>,
    hnd: EpHnd,
}

impl ProxyEp {
    pub async fn new(sub_ep: Ep, cert_digest: CertDigest) -> KitsuneResult<Ep> {
        let logic_chan = LogicChan::new(32);
        let logic_hnd = logic_chan.handle().clone();

        let hnd = ProxyEpHnd::new(
            sub_ep.handle().clone(),
            cert_digest.clone(),
            logic_hnd.clone(),
        );

        let logic = incoming_evt_logic(sub_ep, hnd.clone(), cert_digest, logic_hnd);

        let hnd2 = logic_chan.handle().clone();
        hnd2.capture_logic(logic).await?;

        Ok(Ep(Box::new(ProxyEp {
            logic_chan,
            hnd: EpHnd(hnd),
        })))
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
        EpFactory(Arc::new(ProxyEpFactory {
            tls_config,
            sub_fact,
        }))
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
    use kitsune_p2p_types::tx2::tx2_promote::*;

    async fn build_node(
        mut s_done: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> (tokio::task::JoinHandle<()>, TxUrl, EpHnd) {
        let t = KitsuneTimeout::from_millis(5000);

        let f = tx2_promote(MemBackendAdapt::new(), 32);

        let f = tx2_proxy(f, TlsConfig::new_ephemeral().await.unwrap());

        let mut ep = f.bind("none:", t).await.unwrap();
        let ephnd = ep.handle().clone();
        let addr = ephnd.local_addr().unwrap();

        let join = tokio::task::spawn(async move {
            while let Some(evt) = ep.next().await {
                if let EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) = evt {
                    if data.as_ref() == b"hello" {
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
        let _t_con = t_ep.connect(p_addr.clone(), t).await.unwrap();
        //println!("TGT ADDR = {}", t_addr);

        let t_addr_proxy = proxify_addr(&p_addr, &t_addr);
        //println!("TGT PROXY ADDR = {}", t_addr_proxy);

        const COUNT: usize = 100;

        let mut all_futs = Vec::new();
        for _ in 0..COUNT {
            let (s_done, r_done) = tokio::sync::oneshot::channel();
            let (n_join, _n_addr, n_ep) = build_node(Some(s_done)).await;
            let t_addr_proxy = t_addr_proxy.clone();
            all_futs.push(async move {
                let con = n_ep.connect(t_addr_proxy, t).await.unwrap();
                let mut data = PoolBuf::new();
                data.extend_from_slice(b"hello");
                con.write(0.into(), data, t).await.unwrap();
                r_done.await.unwrap();
                n_ep.close(0, "").await;
                n_join.await.unwrap();
            });
        }

        futures::future::join_all(all_futs).await;

        p_ep.close(0, "").await;
        t_ep.close(0, "").await;

        futures::future::try_join_all(all_tasks).await.unwrap();
    }
}
