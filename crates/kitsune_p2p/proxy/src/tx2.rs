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
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;

/// Configuration for the remote connection portion
/// of tx2 proxy wrapper
#[derive(Debug, Clone)]
pub enum ProxyRemoteType {
    /// Don't connect to a remote proxy
    NoProxy,

    /// Request proxying through this specific remote proxy address
    Specific(TxUrl),

    /// Fetch / configure proxy through bootstrap service
    /// or optionally fallback on specific proxy address
    Bootstrap {
        /// the bootstrap address from which to request proxy_list
        bootstrap_url: TxUrl,

        /// optional specific proxy url fallback
        fallback_proxy_url: Option<TxUrl>,
    },
}

impl Default for ProxyRemoteType {
    fn default() -> Self {
        ProxyRemoteType::NoProxy
    }
}

impl ProxyRemoteType {
    /// Get the appropriate proxy_url (or None) given the config
    pub async fn get_proxy_url(&self) -> Option<TxUrl> {
        match self {
            ProxyRemoteType::NoProxy => None,
            ProxyRemoteType::Specific(proxy_url) => Some(proxy_url.clone()),
            ProxyRemoteType::Bootstrap {
                fallback_proxy_url, ..
            } => {
                // TODO lookup bootstrap proxy_list here
                fallback_proxy_url.clone()
            }
        }
    }
}

/// Configuration for tx2 proxy wrapper
#[non_exhaustive]
#[derive(Default)]
pub struct ProxyConfig {
    /// Tuning Params
    /// Default: None = default.
    pub tuning_params: Option<KitsuneP2pTuningParams>,

    /// If enabled, allow forwarding of messages (proxying)
    /// If you are a proxy server, set this to true.
    /// If you are a client, leave this as the default false.
    /// Default: false.
    pub allow_proxy_fwd: bool,

    /// If Some(addr), we will try to keep an open connection to addr.
    /// The node at addr should forward messages intended for us,
    /// and we will modify our local_addr() function to make that
    /// endpoint our external address.
    pub client_of_remote_proxy: ProxyRemoteType,
}

impl ProxyConfig {
    /// into inner contents with default application
    pub fn split(self) -> KitsuneResult<(KitsuneP2pTuningParams, bool, ProxyRemoteType)> {
        let ProxyConfig {
            tuning_params,
            allow_proxy_fwd,
            client_of_remote_proxy,
        } = self;

        let tuning_params = tuning_params.unwrap_or_default();

        Ok((tuning_params, allow_proxy_fwd, client_of_remote_proxy))
    }
}

/// Wrap a tx2 transport pool adapter with proxy logic.
pub fn tx2_proxy(sub_fact: EpFactory, config: ProxyConfig) -> KitsuneResult<EpFactory> {
    ProxyEpFactory::new(sub_fact, config)
}

// -- private -- //

const PROXY_TYPE_BYTES: usize = 1;
const DIGEST_BYTES: usize = 32;

const PROXY_FWD_MSG: u8 = 0x30;
const PROXY_ROUTE_ERR: u8 = 0xc0;

struct ProxyConHnd {
    uniq: Uniq,
    dir: Tx2ConDir,
    sub_con: ConHnd,
    local_cert: Tx2Cert,
    peer_cert: Tx2Cert,
}

impl std::fmt::Debug for ProxyConHnd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ConHnd").field(&self.uniq).finish()
    }
}

impl ProxyConHnd {
    pub fn new(sub_con: ConHnd, local_cert: Tx2Cert, peer_cert: Tx2Cert) -> ConHnd {
        let uniq = Uniq::default();
        let dir = sub_con.dir();
        let con = Self {
            uniq,
            dir,
            sub_con,
            local_cert,
            peer_cert,
        };
        let con: ConHnd = Arc::new(con);
        con
    }
}

impl AsConHnd for ProxyConHnd {
    fn uniq(&self) -> Uniq {
        self.uniq
    }

    fn dir(&self) -> Tx2ConDir {
        self.dir
    }

    fn is_closed(&self) -> bool {
        self.sub_con.is_closed()
    }

    fn close(&self, _code: u32, _reason: &str) -> BoxFuture<'static, ()> {
        // NOTE
        // we don't want to close the underlying sub_con,
        // it could be shared for proxying...
        // do we want to do *anything*?
        async move {}.boxed()
    }

    fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        let peer_addr = self.sub_con.peer_addr()?;
        promote_addr(&peer_addr, &self.peer_cert)
    }

    fn peer_cert(&self) -> Tx2Cert {
        self.peer_cert.clone()
    }

    fn write(
        &self,
        msg_id: MsgId,
        mut data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'static, KitsuneResult<()>> {
        data.reserve_front(PROXY_TYPE_BYTES + DIGEST_BYTES + DIGEST_BYTES);
        data.prepend_from_slice(&self.local_cert);
        data.prepend_from_slice(&self.peer_cert);
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

#[derive(Clone)]
struct Backoff {
    bo: Arc<AtomicUsize>,
    n: Arc<Notify>,
    init: usize,
    max: usize,
}

impl Backoff {
    pub fn new(init: usize, max: usize) -> Self {
        Self {
            bo: Arc::new(AtomicUsize::new(init)),
            n: Arc::new(Notify::new()),
            init,
            max,
        }
    }

    pub fn reset(&self) {
        self.bo.store(self.init, Ordering::SeqCst);
        self.n.notify_waiters();
    }

    pub fn wait(&self) -> impl std::future::Future<Output = Result<(), ()>> + 'static + Send {
        let bo = self.bo.clone();
        let n = self.n.clone();
        let max = self.max;
        async move {
            // prioritize responsiveness over false positives
            // by capturing the notified future first
            let n_fut = n.notified();
            let bo = bo.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |mut x| {
                x *= 2;
                if x > max {
                    x = max
                }
                Some(x)
            });
            let bo = match bo {
                Err(_) => return Err(()),
                Ok(bo) => bo as u64,
            };
            tokio::select! {
                _ = n_fut => (),
                _ = tokio::time::sleep(std::time::Duration::from_millis(bo)) => (),
            };
            Ok(())
        }
    }
}

struct ProxyEpInner {
    // map peer certs to connection handles
    // so on proxy requests we know who to send to
    // these are !SUB CONS! they should not be returned
    // only store INCOMING connections here
    // outgoing connections should not proxy
    digest_to_sub_con_map: HashMap<Tx2Cert, ConHnd>,

    // allows us to clone Tx2ConHnd items which will share
    // the same Uniq, rather than duplicating handles to the same connection.
    // these are !OUT CONS! they are returned from api requests / events.
    // these are both INCOMING and OUTGOING
    direct_to_final_peer_con_map: HashMap<Uniq, HashMap<Tx2Cert, ConHnd>>,

    backoff: Backoff,
}

impl ProxyEpInner {
    pub fn get_con_hnd(
        &mut self,
        sub_con: ConHnd,
        local_cert: Tx2Cert,
        final_peer_cert: Tx2Cert,
    ) -> KitsuneResult<(bool, ConHnd)> {
        let direct_peer = sub_con.uniq();
        let inner_map = self
            .direct_to_final_peer_con_map
            .entry(direct_peer)
            .or_insert_with(HashMap::new);
        let mut did_insert = false;
        let con = {
            let did_insert = &mut did_insert;
            inner_map
                .entry(final_peer_cert.clone())
                .or_insert_with(move || {
                    *did_insert = true;
                    ProxyConHnd::new(sub_con, local_cert, final_peer_cert)
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
    cur_proxy_url: Share<Option<ProxyUrl>>,
}

async fn get_con_hnd(
    inner: &Share<ProxyEpInner>,
    logic_hnd: LogicChanHandle<EpEvent>,
    sub_con: ConHnd,
    local_cert: Tx2Cert,
    peer_cert: Tx2Cert,
    is_outgoing: bool,
) -> KitsuneResult<ConHnd> {
    let (did_insert, con) =
        inner.share_mut(move |i, _| i.get_con_hnd(sub_con, local_cert, peer_cert))?;
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
        backoff: Backoff,
        cur_proxy_url: Share<Option<ProxyUrl>>,
    ) -> KitsuneResult<Arc<ProxyEpHnd>> {
        let local_cert = sub_ep_hnd.local_cert();
        Ok(Arc::new(ProxyEpHnd {
            sub_ep_hnd,
            local_cert,
            logic_hnd,
            inner: Share::new(ProxyEpInner {
                digest_to_sub_con_map: HashMap::new(),
                direct_to_final_peer_con_map: HashMap::new(),
                backoff,
            }),
            cur_proxy_url,
        }))
    }
}

impl AsEpHnd for ProxyEpHnd {
    fn debug(&self) -> serde_json::Value {
        let addr = self.local_addr();
        match self.inner.share_mut(|i, _| {
            let proxy_list = i
                .digest_to_sub_con_map
                .keys()
                .map(|k| format!("{:?}", k))
                .collect::<Vec<_>>();
            Ok(serde_json::json!({
                "type": "tx2_proxy",
                "state": "open",
                "addr": addr?,
                "proxy_count": i.digest_to_sub_con_map.len(),
                "proxy_list": proxy_list,
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
        if let Ok(Some(proxy_url)) = self.cur_proxy_url.share_ref(|r| Ok(r.clone())) {
            let proxy_addr: TxUrl = ProxyUrl::new(
                proxy_url.as_base().as_str(),
                self.local_cert.as_digest().clone(),
            )
            .map_err(KitsuneError::other)?
            .as_str()
            .into();
            Ok(proxy_addr)
        } else {
            let local_addr = self.sub_ep_hnd.local_addr()?;
            let proxy_addr: TxUrl =
                ProxyUrl::new(local_addr.as_str(), self.local_cert.as_digest().clone())
                    .map_err(KitsuneError::other)?
                    .as_str()
                    .into();
            Ok(proxy_addr)
        }
    }

    fn local_cert(&self) -> Tx2Cert {
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
        // NOTE
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
        let peer_cert = purl.digest().into();
        if peer_cert == self.local_cert {
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
            get_con_hnd(&inner, logic_hnd, sub_con, local_cert, peer_cert, true).await
        }
        .boxed()
    }
}

async fn close_ep(hnd: &Arc<ProxyEpHnd>, logic_hnd: &LogicChanHandle<EpEvent>) {
    let _ = hnd.inner.share_mut(|_, c| {
        *c = true;
        Ok(())
    });
    let _ = logic_hnd.emit(EpEvent::EndpointClosed).await;
    logic_hnd.close();
}

async fn incoming_evt_logic(
    tuning_params: KitsuneP2pTuningParams,
    allow_proxy_fwd: bool,
    sub_ep: Ep,
    hnd: Arc<ProxyEpHnd>,
    logic_hnd: LogicChanHandle<EpEvent>,
    cur_proxy_url: Share<Option<ProxyUrl>>,
) {
    let local_cert = sub_ep.handle().local_cert();
    let local_cert = &local_cert;
    let tuning_params = &tuning_params;
    let cur_proxy_url = &cur_proxy_url;

    // Benchmarks showed a slight slowdown when using semaphore count tasks
    // instead of for_each_concurrent... but maybe other problems caused that?
    sub_ep
        .for_each_concurrent(tuning_params.concurrent_limit_per_thread, |evt| async {
            incoming_evt_handle(
                tuning_params,
                allow_proxy_fwd,
                evt,
                local_cert.clone(),
                &hnd,
                &logic_hnd,
                cur_proxy_url,
            )
            .await;
        })
        .await;

    tracing::warn!("proxy loop end");
}

async fn ensure_proxy_register(
    inner: &Share<ProxyEpInner>,
    logic_hnd: &LogicChanHandle<EpEvent>,
    local_cert: &Tx2Cert,
    sub_con: ConHnd,
    cur_proxy_url: &Share<Option<ProxyUrl>>,
) -> KitsuneResult<()> {
    // first make sure we are not connecting to ourselves
    // (or some node that somehow insecurely is using the same cert)
    let peer_cert = sub_con.peer_cert();
    if &peer_cert == local_cert {
        close_connection(
            inner,
            logic_hnd,
            sub_con,
            500,
            "refusing connection with matching cert",
            cur_proxy_url,
        )
        .await;
        tracing::warn!("refusing connection with matching cert");
        return Err(().into());
    }

    // we don't register outgoing connections for proxy-ing
    // that doesn't make any sense.
    if let Tx2ConDir::Outgoing = sub_con.dir() {
        return Ok(());
    }

    let _ = inner.share_mut(move |i, _| {
        match i.digest_to_sub_con_map.entry(peer_cert.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if e.get().uniq() != sub_con.uniq() {
                    tracing::warn!(?peer_cert, "REPLACE EXISTING CONNECTION!");
                    e.insert(sub_con);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(sub_con);
            }
        }
        Ok(())
    });
    Ok(())
}

async fn incoming_evt_handle(
    tuning_params: &KitsuneP2pTuningParams,
    allow_proxy_fwd: bool,
    evt: EpEvent,
    local_cert: Tx2Cert,
    hnd: &Arc<ProxyEpHnd>,
    logic_hnd: &LogicChanHandle<EpEvent>,
    cur_proxy_url: &Share<Option<ProxyUrl>>,
) {
    //println!("EVT: {:?}", evt);
    use EpEvent::*;
    match evt {
        OutgoingConnection(EpConnection { con: sub_con, .. }) => {
            let _ =
                ensure_proxy_register(&hnd.inner, logic_hnd, &local_cert, sub_con, cur_proxy_url)
                    .await;
        }
        IncomingConnection(EpConnection { con: sub_con, .. }) => {
            let _ =
                ensure_proxy_register(&hnd.inner, logic_hnd, &local_cert, sub_con, cur_proxy_url)
                    .await;
        }
        IncomingError(_) => unreachable!(), // currently no lower layers invoke this
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
            if ensure_proxy_register(
                &hnd.inner,
                logic_hnd,
                &local_cert,
                sub_con.clone(),
                cur_proxy_url,
            )
            .await
            .is_err()
            {
                return;
            };
            match data[0] {
                PROXY_FWD_MSG => {
                    const SRC_START: usize = PROXY_TYPE_BYTES + DIGEST_BYTES;
                    const SRC_END: usize = SRC_START + DIGEST_BYTES;

                    const DEST_START: usize = PROXY_TYPE_BYTES;
                    const DEST_END: usize = DEST_START + DIGEST_BYTES;
                    let src_cert = data[SRC_START..SRC_END].to_vec().into();
                    let dest_cert = data[DEST_START..DEST_END].to_vec().into();
                    if dest_cert == hnd.local_cert {
                        // this data is destined for US!
                        data.cheap_move_start(SRC_END);
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
                        let dest = if !allow_proxy_fwd {
                            tracing::error!("received fwd request on, but proxy fwd is disallowed");
                            Err("proxy fwd disallowed".into())
                        } else {
                            hnd.inner.share_mut(|i, _| {
                                Ok(i.digest_to_sub_con_map.get(&dest_cert).cloned())
                            })
                        };
                        if let Err(e) = match dest {
                            Ok(Some(d_sub_con)) => {
                                write_to_sub_con(
                                    tuning_params,
                                    &hnd.inner,
                                    logic_hnd,
                                    d_sub_con,
                                    msg_id,
                                    data,
                                    cur_proxy_url,
                                )
                                .await
                            }
                            Ok(None) => {
                                Err(format!("Invalid Proxy Target: {:?}", dest_cert).into())
                            }
                            Err(e) => Err(e),
                        } {
                            tracing::warn!("Proxy Fwd Error: {:?}", e);
                            let new_msg_id = if msg_id.is_notify() {
                                0.into()
                            } else {
                                msg_id.as_res()
                            };
                            let mut data = PoolBuf::new();
                            data.extend_from_slice(format!("{:?}", e).as_bytes());
                            data.prepend_from_slice(&local_cert);
                            data.prepend_from_slice(&[PROXY_ROUTE_ERR]);
                            let _ = write_to_sub_con(
                                tuning_params,
                                &hnd.inner,
                                logic_hnd,
                                sub_con,
                                new_msg_id,
                                data,
                                cur_proxy_url,
                            )
                            .await;
                        }
                    }
                }
                PROXY_ROUTE_ERR => {
                    const SRC_START: usize = PROXY_TYPE_BYTES;
                    const SRC_END: usize = SRC_START + DIGEST_BYTES;
                    let src_cert = data[SRC_START..SRC_END].to_vec().into();
                    data.cheap_move_start(SRC_END);

                    let url = promote_addr(&base_url, &src_cert).unwrap();
                    let con = match get_con_hnd(
                        &hnd.inner,
                        logic_hnd.clone(),
                        sub_con,
                        local_cert,
                        src_cert,
                        false,
                    )
                    .await
                    {
                        Err(_) => return,
                        Ok(con) => con,
                    };
                    let err = String::from_utf8_lossy(data.as_ref());
                    let err: &str = &err;
                    let evt = EpEvent::IncomingError(EpIncomingError {
                        con,
                        url,
                        msg_id,
                        err: err.into(),
                    });
                    let _ = logic_hnd.emit(evt).await;
                }
                b => {
                    let reason = format!("Invalid Proxy Byte: {}, closing connection", b);
                    tracing::warn!("{}", reason);
                    close_connection(&hnd.inner, logic_hnd, sub_con, 500, &reason, cur_proxy_url)
                        .await;
                }
            }
        }
        ConnectionClosed(EpConnectionClosed {
            con, code, reason, ..
        }) => {
            close_connection_inner(&hnd.inner, logic_hnd, con, code, &reason, cur_proxy_url).await;
        }
        Error(e) => {
            let _ = logic_hnd.emit(Error(e)).await;
        }
        EndpointClosed => {
            close_ep(hnd, logic_hnd).await;
        }
    }
}

async fn write_to_sub_con(
    tuning_params: &KitsuneP2pTuningParams,
    inner: &Share<ProxyEpInner>,
    logic_hnd: &LogicChanHandle<EpEvent>,
    sub_con: ConHnd,
    msg_id: MsgId,
    data: PoolBuf,
    cur_proxy_url: &Share<Option<ProxyUrl>>,
) -> KitsuneResult<()> {
    let t = tuning_params.implicit_timeout();
    if let Err(e) = sub_con.write(msg_id, data, t).await {
        let reason = format!("{:?}", e);
        close_connection(inner, logic_hnd, sub_con, 500, &reason, cur_proxy_url).await;
        return Err(e);
    }
    Ok(())
}

async fn close_connection(
    inner: &Share<ProxyEpInner>,
    logic_hnd: &LogicChanHandle<EpEvent>,
    sub_con: ConHnd,
    code: u32,
    reason: &str,
    cur_proxy_url: &Share<Option<ProxyUrl>>,
) {
    let c_fut = sub_con.close(code, reason);
    close_connection_inner(inner, logic_hnd, sub_con, code, reason, cur_proxy_url).await;
    c_fut.await;
}

async fn close_connection_inner(
    inner: &Share<ProxyEpInner>,
    logic_hnd: &LogicChanHandle<EpEvent>,
    sub_con: ConHnd,
    code: u32,
    reason: &str,
    cur_proxy_url: &Share<Option<ProxyUrl>>,
) {
    let peer_dir = sub_con.dir();
    let peer_cert = sub_con.peer_cert();
    let direct_peer = sub_con.uniq();

    let inner_res = inner.share_mut(|i, _| {
        // if this is an INCOMING connection, remove it from our proxy list
        if let Tx2ConDir::Incoming = peer_dir {
            i.digest_to_sub_con_map.remove(&peer_cert);
        }

        // remove all out cons associated with this exact connection
        Ok((
            i.backoff.clone(),
            i.direct_to_final_peer_con_map.remove(&direct_peer),
        ))
    });

    let kill_cons = match inner_res {
        Ok((backoff, kill_cons)) => {
            if let Ok(Some(proxy_url)) = cur_proxy_url.share_ref(|r| Ok(r.clone())) {
                let proxy_cert = Tx2Cert::from(proxy_url.digest());
                if proxy_cert == peer_cert {
                    // reset our client proxy connection check timer
                    // so we'll try to reconnect
                    backoff.reset();
                }
            }

            match kill_cons {
                Some(kill_cons) => kill_cons,
                None => return,
            }
        }
        _ => return,
    };

    for (_, c) in kill_cons.into_iter() {
        let url = match c.peer_addr() {
            Ok(url) => url,
            _ => continue,
        };
        let evt = EpEvent::ConnectionClosed(EpConnectionClosed {
            con: c,
            url,
            code,
            reason: reason.to_string(),
        });
        let _ = logic_hnd.emit(evt).await;
    }
}

struct ProxyEp {
    logic_chan: LogicChan<EpEvent>,
    hnd: EpHnd,
}

impl ProxyEp {
    pub async fn new(
        sub_ep: Ep,
        tuning_params: KitsuneP2pTuningParams,
        allow_proxy_fwd: bool,
        client_of_remote_proxy: ProxyRemoteType,
    ) -> KitsuneResult<Ep> {
        // this isn't something that needs to be configurable,
        // because it's entirely dependent on the code written here
        // we only ever capture two logic closures
        // so technically, it only really would need to be 2.
        const LOGIC_CHAN_LIMIT: usize = 32;

        let cur_proxy_url = Share::new(None);

        let logic_chan = LogicChan::new(LOGIC_CHAN_LIMIT);
        let logic_hnd = logic_chan.handle().clone();

        let backoff = Backoff::new(10, 5000);

        let hnd = ProxyEpHnd::new(
            sub_ep.handle().clone(),
            logic_hnd.clone(),
            backoff.clone(),
            cur_proxy_url.clone(),
        )?;

        let logic = incoming_evt_logic(
            tuning_params.clone(),
            allow_proxy_fwd,
            sub_ep,
            hnd.clone(),
            logic_hnd,
            cur_proxy_url.clone(),
        );

        let l_hnd = logic_chan.handle().clone();
        l_hnd.capture_logic(logic).await?;

        {
            let hnd = hnd.clone();
            l_hnd
                .capture_logic(async move {
                    loop {
                        if backoff.wait().await.is_err() {
                            break;
                        }

                        if let Some(proxy_url) = client_of_remote_proxy.get_proxy_url().await {
                            let _ = cur_proxy_url.share_mut(|r, _| {
                                *r = Some(ProxyUrl::from(proxy_url.as_str()));
                                Ok(())
                            });
                            let timeout = tuning_params.implicit_timeout();
                            let _ = hnd.get_connection(proxy_url, timeout).await;
                        }
                    }
                })
                .await?;
        }

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
    allow_proxy_fwd: bool,
    client_of_remote_proxy: ProxyRemoteType,
    sub_fact: EpFactory,
}

impl ProxyEpFactory {
    pub fn new(sub_fact: EpFactory, config: ProxyConfig) -> KitsuneResult<EpFactory> {
        let (tuning_params, allow_proxy_fwd, client_of_remote_proxy) = config.split()?;
        let fact: EpFactory = Arc::new(ProxyEpFactory {
            tuning_params,
            allow_proxy_fwd,
            client_of_remote_proxy,
            sub_fact,
        });
        Ok(fact)
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
        let allow_proxy_fwd = self.allow_proxy_fwd;
        let client_of_remote_proxy = self.client_of_remote_proxy.clone();
        async move {
            let sub_ep = fut.await?;
            ProxyEp::new(
                sub_ep,
                tuning_params,
                allow_proxy_fwd,
                client_of_remote_proxy,
            )
            .await
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
        expect_err: bool,
    ) -> (tokio::task::JoinHandle<KitsuneResult<()>>, TxUrl, EpHnd) {
        let t = KitsuneTimeout::from_millis(5000);

        let f = tx2_mem_adapter(MemConfig::default()).await.unwrap();
        let f = tx2_pool_promote(f, Default::default());

        let mut conf = super::ProxyConfig::default();
        conf.allow_proxy_fwd = true;
        let f = tx2_proxy(f, conf).unwrap();

        let mut ep = f.bind("none:".into(), t).await.unwrap();
        let ephnd = ep.handle().clone();
        let addr = ephnd.local_addr().unwrap();

        let join = kitsune_p2p_types::metrics::metric_task(async move {
            while let Some(evt) = ep.next().await {
                match evt {
                    EpEvent::IncomingData(EpIncomingData { con, mut data, .. }) => {
                        if expect_err {
                            panic!("got response, expected err");
                        }

                        if data.as_ref() == b"" {
                            // pass - this is the proxy hello
                        } else if data.as_ref() == b"hello" {
                            data.clear();
                            data.extend_from_slice(b"world");
                            con.write(0.into(), data, t).await.unwrap();
                        } else if data.as_ref() == b"world" {
                            if let Some(s_done) = s_done.take() {
                                let _ = s_done.send(());
                                return Ok(());
                            }
                        } else {
                            panic!("unexpected: {}", String::from_utf8_lossy(&data));
                        }
                    }
                    EpEvent::IncomingError(EpIncomingError { err, .. }) => {
                        if !expect_err {
                            panic!("err: {:?}", err);
                        }
                        if let Some(s_done) = s_done.take() {
                            let _ = s_done.send(());
                            return Ok(());
                        }
                    }
                    _ => (),
                }
            }
            KitsuneResult::Ok(())
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
    async fn test_tx2_route_err() {
        observability::test_run().ok();
        let t = KitsuneTimeout::from_millis(5000);
        let mut all_tasks = Vec::new();

        let (p_join, p_addr, p_ep) = build_node(None, true).await;
        all_tasks.push(p_join);

        let fake_tgt: Tx2Cert = vec![0xdb; 32].into();
        let fake_tgt = ProxyUrl::new(
            ProxyUrl::from(p_addr.as_str()).as_base().as_str(),
            fake_tgt.into(),
        )
        .unwrap();
        let fake_tgt = fake_tgt.as_str().into();
        println!("Fake Tgt: {:?}", fake_tgt);

        let (s_done, r_done) = tokio::sync::oneshot::channel();
        let (n_join, _n_addr, n_ep) = build_node(Some(s_done), true).await;

        let mut data = PoolBuf::new();
        data.extend_from_slice(b"hello");
        n_ep.write(fake_tgt, 0.into(), data, t).await.unwrap();
        r_done.await.unwrap();
        n_ep.close(0, "").await;
        n_join.await.unwrap().unwrap();

        p_ep.close(0, "").await;

        futures::future::try_join_all(all_tasks).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tx2_proxy() {
        observability::test_run().ok();

        let t = KitsuneTimeout::from_millis(5000);

        let mut all_tasks = Vec::new();

        let (p_join, p_addr, p_ep) = build_node(None, false).await;
        all_tasks.push(p_join);
        //println!("PROXY ADDR = {}", p_addr);
        //println!("PROXY: {:?}", p_ep.local_cert().unwrap());

        let (t_join, t_addr, t_ep) = build_node(None, false).await;
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
            let (n_join, _n_addr, n_ep) = build_node(Some(s_done), false).await;
            //println!("N: {:?}", n_ep.local_cert().unwrap());

            let t_addr_proxy = t_addr_proxy.clone();
            all_futs.push(async move {
                let mut data = PoolBuf::new();
                data.extend_from_slice(b"hello");
                n_ep.write(t_addr_proxy, 0.into(), data, t).await.unwrap();
                r_done.await.unwrap();
                n_ep.close(0, "").await;
                n_join.await.unwrap().unwrap();
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
