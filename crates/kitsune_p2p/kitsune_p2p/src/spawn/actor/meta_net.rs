// because of feature flipping
#![allow(dead_code)]
#![allow(irrefutable_let_patterns)]
#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unreachable_patterns)]
#![allow(clippy::needless_return)]
#![allow(clippy::blocks_in_if_conditions)]
//! Networking abstraction to handle feature flipping.

use crate::wire::WireData;
use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

#[cfg(feature = "tx2")]
use kitsune_p2p_proxy::tx2::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_transport_quic::tx2::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_api::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_pool_promote::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::tx2_restart_adapter::*;
#[cfg(feature = "tx2")]
use kitsune_p2p_types::tx2::*;

use crate::spawn::actor::InternalSender;
use crate::spawn::KitsuneP2pEvent;
use crate::spawn::PutAgentInfoSignedEvt;
use crate::types::event::KitsuneP2pEventSender;
use kitsune_p2p_block::BlockTargetId;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::*;
use opentelemetry_api::metrics::Histogram;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::spawn::actor::UNAUTHORIZED_DISCONNECT_CODE;
use crate::spawn::actor::UNAUTHORIZED_DISCONNECT_REASON;

kitsune_p2p_types::write_codec_enum! {
    /// KitsuneP2p WebRTC wrapper enum.
    codec WireWrap {
        /// Notification not needing a response.
        Notify(0x00) {
            msg_id.0: u64,
            data.1: WireData,
        },

        /// Request that expects a response.
        Request(0x10) {
            msg_id.0: u64,
            data.1: WireData,
        },

        /// Response to a previous request.
        Response(0x11) {
            msg_id.0: u64,
            data.1: WireData,
        },
    }
}

fn next_msg_id() -> u64 {
    static MSG_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    // MAYBE - track these message ids at the connection level
    // to prevent mismatches
    MSG_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub type RespondFut = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static + Send>>;

pub type Respond = Box<dyn FnOnce(wire::Wire) -> RespondFut + 'static + Send>;

/// Events emitted by a meta net instance.
pub enum MetaNetEvt {
    /// A connection has been established.
    Connected {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,
    },

    /// A connection has been closed.
    Disconnected {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,
    },

    /// An incoming request expecting a response.
    Request {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,

        /// The request data sent by the remote peer.
        data: wire::Wire,

        /// Respond to this request.
        respond: Respond,
    },

    /// An incoming notification that doesn't require a direct response.
    Notify {
        /// Identifies the remote peer.
        remote_url: String,

        /// Handle to the connection.
        con: MetaNetCon,

        /// The request data sent by the remote peer.
        data: wire::Wire,
    },
}

impl MetaNetEvt {
    pub fn con(&self) -> &MetaNetCon {
        match self {
            MetaNetEvt::Connected { con, .. }
            | MetaNetEvt::Disconnected { con, .. }
            | MetaNetEvt::Request { con, .. }
            | MetaNetEvt::Notify { con, .. } => con,
        }
    }

    pub fn maybe_space(&self) -> Option<Arc<KitsuneSpace>> {
        match self {
            MetaNetEvt::Request { data, .. } | MetaNetEvt::Notify { data, .. } => {
                data.maybe_space()
            }
            MetaNetEvt::Connected { .. } | MetaNetEvt::Disconnected { .. } => None,
        }
    }
}

pub enum MetaNetAuth {
    Authorized,
    UnauthorizedIgnore,
    UnauthorizedDisconnect,
}

async fn node_is_authorized(host: &HostApi, node_id: Arc<[u8; 32]>, now: Timestamp) -> MetaNetAuth {
    match host.is_blocked(BlockTargetId::Node(node_id), now).await {
        Ok(true) => MetaNetAuth::UnauthorizedDisconnect,
        Ok(false) => MetaNetAuth::Authorized,
        Err(_) => MetaNetAuth::UnauthorizedIgnore,
    }
}

pub async fn nodespace_is_authorized(
    host: &HostApi,
    node_id: Arc<[u8; 32]>,
    maybe_space: Option<Arc<KitsuneSpace>>,
    now: Timestamp,
) -> MetaNetAuth {
    if let Some(space) = maybe_space {
        match node_is_authorized(host, node_id.clone(), now).await {
            MetaNetAuth::Authorized => {
                match host
                    .is_blocked(BlockTargetId::NodeSpace(node_id, space), now)
                    .await
                {
                    Ok(true) => MetaNetAuth::UnauthorizedIgnore,
                    Ok(false) => MetaNetAuth::Authorized,
                    Err(_) => MetaNetAuth::UnauthorizedIgnore,
                }
            }
            unauthorized => unauthorized,
        }
    } else {
        MetaNetAuth::Authorized
    }
}

pub type MetaNetEvtRecv = futures::channel::mpsc::Receiver<MetaNetEvt>;

type ResStore = Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<wire::Wire>>>>;

struct MetricSendGuard {
    rem_id: tx5::Id,
    is_error: bool,
    byte_count: u64,
    start_time: std::time::Instant,
}

impl MetricSendGuard {
    pub fn new(rem_id: tx5::Id, byte_count: u64) -> Self {
        Self {
            rem_id,
            is_error: true,
            byte_count,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn set_is_error(&mut self, is_error: bool) {
        self.is_error = is_error;
    }
}

impl Drop for MetricSendGuard {
    fn drop(&mut self) {
        crate::metrics::METRIC_MSG_OUT_BYTE.record(
            self.byte_count,
            &[
                opentelemetry_api::KeyValue::new("remote_id", self.rem_id.to_string()),
                opentelemetry_api::KeyValue::new("is_error", self.is_error),
            ],
        );
        crate::metrics::METRIC_MSG_OUT_TIME.record(
            self.start_time.elapsed().as_secs_f64(),
            &[
                opentelemetry_api::KeyValue::new("remote_id", self.rem_id.to_string()),
                opentelemetry_api::KeyValue::new("is_error", self.is_error),
            ],
        );
    }
}

#[derive(Debug, Clone)]
pub enum MetaNetCon {
    #[cfg(feature = "tx2")]
    Tx2(Tx2ConHnd<wire::Wire>, HostApi),

    #[cfg(feature = "tx5")]
    Tx5 {
        host: HostApi,
        ep: tx5::Ep,
        rem_url: tx5::Tx5Url,
        res: ResStore,
        tun: KitsuneP2pTuningParams,
    },

    #[cfg(test)]
    Test {
        state: Arc<parking_lot::RwLock<MetaNetConTest>>,
    },
}

impl PartialEq for MetaNetCon {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(feature = "tx2")]
            (MetaNetCon::Tx2(a, _), MetaNetCon::Tx2(b, _)) => a == b,
            #[cfg(feature = "tx5")]
            (MetaNetCon::Tx5 { ep: a, .. }, MetaNetCon::Tx5 { ep: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Eq for MetaNetCon {}

impl MetaNetCon {
    pub async fn close(&self, code: u32, reason: &str) {
        #[cfg(test)]
        {
            if let MetaNetCon::Test { state } = self {
                state.write().closed = true;
                return;
            }
        }

        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con, _) = self {
                con.close(code, reason).await;
                return;
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNetCon::Tx5 {
                ep, rem_url, tun, ..
            } = self
            {
                ep.ban(rem_url.id().unwrap(), tun.tx5_ban_time());
                return;
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        #[cfg(test)]
        {
            if let MetaNetCon::Test { state } = self {
                return state.read().closed;
            }
        }

        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con, _) = self {
                return con.is_closed();
            }
        }

        #[cfg(feature = "tx5")]
        {
            // NOTE - tx5 connections are never exactly "closed"
            //        since it's more of a message queue...
            return false;
        }

        true
    }

    async fn wire_is_authorized(&self, payload: &wire::Wire, now: Timestamp) -> MetaNetAuth {
        match self {
            MetaNetCon::Tx5 { host, .. } | MetaNetCon::Tx2(_, host) => {
                nodespace_is_authorized(host, self.peer_id(), payload.maybe_space(), now).await
            }
            #[cfg(test)]
            MetaNetCon::Test { .. } => MetaNetAuth::Authorized,
        }
    }

    pub async fn notify(&self, payload: &wire::Wire, timeout: KitsuneTimeout) -> KitsuneResult<()> {
        let start = std::time::Instant::now();
        let msg_id = next_msg_id();

        let result = async move {
            match self.wire_is_authorized(payload, Timestamp::now()).await {
                MetaNetAuth::Authorized => {
                    #[cfg(test)]
                    {
                        if let MetaNetCon::Test { state } = self {
                            let mut state = state.write();
                            state.notify_call_count += 1;

                            return if state.notify_succeed {
                                Ok(())
                            } else {
                                Err("Test error while notifying".into())
                            };
                        }
                    }

                    #[cfg(feature = "tx2")]
                    {
                        if let MetaNetCon::Tx2(con, _) = self {
                            return con.notify(payload, timeout).await;
                        }
                    }

                    #[cfg(feature = "tx5")]
                    {
                        if let MetaNetCon::Tx5 { ep, rem_url, .. } = self {
                            let wire = payload.encode_vec().map_err(KitsuneError::other)?;
                            let wrap = WireWrap::notify(msg_id, WireData(wire));

                            let data = wrap.encode_vec().map_err(KitsuneError::other)?;

                            let mut metric_guard =
                                MetricSendGuard::new(rem_url.id().unwrap(), data.len() as u64);

                            ep.send(rem_url.clone(), data.as_slice())
                                .await
                                .map_err(KitsuneError::other)?;

                            metric_guard.set_is_error(false);

                            return Ok(());
                        }
                    }

                    return Err("invalid features".into());
                }
                MetaNetAuth::UnauthorizedIgnore => {
                    return Ok(());
                }
                MetaNetAuth::UnauthorizedDisconnect => {
                    self.close(UNAUTHORIZED_DISCONNECT_CODE, UNAUTHORIZED_DISCONNECT_REASON)
                        .await;
                    return Ok(());
                }
            }
        }
        .await;

        let elapsed_s = start.elapsed().as_secs_f64();

        tracing::trace!(%elapsed_s, %msg_id, ?payload, ?result, "sent notify");

        result
    }

    pub async fn request(
        &self,
        payload: &wire::Wire,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<wire::Wire> {
        let start = std::time::Instant::now();
        let msg_id = next_msg_id();

        tracing::trace!(?payload, "initiating request");

        let result = async move {
            match self.wire_is_authorized(payload, Timestamp::now()).await {
                MetaNetAuth::Authorized => {
                    #[cfg(feature = "tx2")]
                    {
                        if let MetaNetCon::Tx2(con, _) = self {
                            return con.request(payload, timeout).await;
                        }
                    }

                    #[cfg(feature = "tx5")]
                    {
                        if let MetaNetCon::Tx5 {
                            ep,
                            rem_url,
                            res: res_store,
                            ..
                        } = self
                        {
                            let (s, r) = tokio::sync::oneshot::channel();
                            res_store.lock().insert(msg_id, s);

                            let res_store = res_store.clone();
                            tokio::task::spawn(async move {
                                tokio::time::sleep(timeout.time_remaining()).await;
                                res_store.lock().remove(&msg_id);
                            });

                            let wire = payload.encode_vec().map_err(KitsuneError::other)?;
                            let wrap = WireWrap::request(msg_id, WireData(wire));
                            let data = wrap.encode_vec().map_err(KitsuneError::other)?;

                            let mut metric_guard =
                                MetricSendGuard::new(rem_url.id().unwrap(), data.len() as u64);

                            ep.send(rem_url.clone(), data.as_slice())
                                .await
                                .map_err(KitsuneError::other)?;

                            let resp = r.await.map_err(|_| KitsuneError::other("timeout"))?;

                            metric_guard.set_is_error(false);
                            return Ok(resp);
                        }
                    }

                    return Err("invalid features".into());
                }
                MetaNetAuth::UnauthorizedIgnore => {
                    return Err(KitsuneErrorKind::Unauthorized.into());
                }
                MetaNetAuth::UnauthorizedDisconnect => {
                    self.close(UNAUTHORIZED_DISCONNECT_CODE, UNAUTHORIZED_DISCONNECT_REASON)
                        .await;
                    return Err(KitsuneErrorKind::Unauthorized.into());
                }
            }
        }
        .await;

        let elapsed_s = start.elapsed().as_secs_f64();

        tracing::trace!(%elapsed_s, %msg_id, ?payload, ?result, "sent request");

        result
    }

    pub fn peer_id(&self) -> Arc<[u8; 32]> {
        #[cfg(test)]
        {
            if let MetaNetCon::Test { state } = self {
                return state.read().id();
            }
        }

        #[cfg(feature = "tx2")]
        {
            if let MetaNetCon::Tx2(con, _) = self {
                return con.peer_cert().into();
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNetCon::Tx5 { rem_url, .. } = self {
                let id = rem_url.id().unwrap();
                return Arc::new(id.0);
            }
        }

        panic!("invalid features");
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct MetaNetConTest {
    pub id: Arc<[u8; 32]>,
    pub closed: bool,

    pub notify_succeed: bool,
    pub notify_call_count: usize,
}

#[cfg(test)]
impl Default for MetaNetConTest {
    fn default() -> Self {
        Self {
            id: Arc::new([0; 32]),
            closed: false,
            notify_succeed: true,
            notify_call_count: 0,
        }
    }
}

#[cfg(test)]
impl MetaNetConTest {
    pub fn new_with_id(id: u8) -> Self {
        Self {
            id: Arc::new(vec![id; 32].try_into().unwrap()),
            ..Default::default()
        }
    }

    pub fn id(&self) -> Arc<[u8; 32]> {
        self.id.clone()
    }
}

/// Networking abstraction to handle feature flipping.
#[derive(Clone)]
pub enum MetaNet {
    /// Tx2 Abstraction
    #[cfg(feature = "tx2")]
    Tx2(Tx2EpHnd<wire::Wire>, HostApi),

    /// Tx5 Abstraction
    #[cfg(feature = "tx5")]
    Tx5 {
        host: HostApi,
        ep: tx5::Ep,
        url: tx5::Tx5Url,
        res: ResStore,
        tun: KitsuneP2pTuningParams,
    },
}

impl MetaNet {
    /// Construct abstraction with tx2 backend.
    #[cfg(feature = "tx2")]
    pub async fn new_tx2(
        host: HostApi,
        config: KitsuneP2pConfig,
        tls_config: kitsune_p2p_types::tls::TlsConfig,
        metrics: Tx2ApiMetrics,
    ) -> KitsuneP2pResult<(Self, MetaNetEvtRecv)> {
        let tuning_params = config.tuning_params.clone();
        let (mut evt_send, evt_recv) =
            futures::channel::mpsc::channel(tuning_params.concurrent_limit_per_thread);

        let tx2_conf = config.to_tx2().map_err(KitsuneP2pError::other)?;

        let mut is_mock = false;

        // set up our backend based on config
        let (f, bind_to): (_, kitsune_p2p_types::tx2::tx2_utils::TxUrl) = match tx2_conf.backend {
            KitsuneP2pTx2Backend::Mem => {
                let mut conf = MemConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_mem_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    "none:".into(),
                )
            }
            /*
            KitsuneP2pTx2Backend::Quic { bind_to } => {
                let mut conf = QuicConfig::default();
                conf.tls = Some(tls_config.clone());
                conf.tuning_params = Some(config.tuning_params.clone());
                (
                    tx2_quic_adapter(conf)
                        .await
                        .map_err(KitsuneP2pError::other)?,
                    bind_to,
                )
            }
            */
            KitsuneP2pTx2Backend::Mock { mock_network } => {
                is_mock = true;
                (mock_network, "none:".into())
            }
        };

        // wrap in restart logic
        let f = tx2_restart_adapter(f);

        // convert to frontend
        let f = tx2_pool_promote(f, config.tuning_params.clone());

        // wrap in proxy
        let f = if !is_mock {
            let mut conf = kitsune_p2p_proxy::tx2::ProxyConfig::default();
            conf.tuning_params = Some(config.tuning_params.clone());
            match tx2_conf.use_proxy {
                KitsuneP2pTx2ProxyConfig::NoProxy => (),
                KitsuneP2pTx2ProxyConfig::Specific(proxy_url) => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Specific(proxy_url);
                }
                KitsuneP2pTx2ProxyConfig::Bootstrap {
                    bootstrap_url,
                    fallback_proxy_url,
                } => {
                    conf.client_of_remote_proxy = ProxyRemoteType::Bootstrap {
                        bootstrap_url,
                        fallback_proxy_url,
                    };
                    conf.proxy_from_bootstrap_cb = Arc::new(|bootstrap_url| {
                        Box::pin(async move {
                            match crate::spawn::actor::bootstrap::proxy_list(
                                bootstrap_url.into(),
                                crate::spawn::actor::bootstrap::BootstrapNet::Tx2,
                            )
                            .await
                            {
                                Ok(mut proxy_list) => {
                                    if proxy_list.is_empty() {
                                        return None;
                                    }
                                    use rand::Rng;
                                    Some(
                                        proxy_list
                                            .remove(
                                                rand::thread_rng().gen_range(0..proxy_list.len()),
                                            )
                                            .into(),
                                    )
                                }
                                _ => None,
                            }
                        })
                    });
                }
            }

            tx2_proxy(f, conf)?
        } else {
            f
        };

        // wrap in api
        let f = tx2_api(f, metrics);

        // bind local endpoint
        let mut ep = f
            .bind(bind_to, config.tuning_params.implicit_timeout())
            .await
            .map_err(KitsuneP2pError::other)?;

        // capture endpoint handle
        let ep_hnd = ep.handle().clone();

        let return_host = host.clone();
        tokio::task::spawn(async move {
            let tuning_params = &tuning_params;
            while let Some(evt) = ep.next().await {
                match evt {
                    Tx2EpEvent::OutgoingConnection(Tx2EpConnection { con, url }) => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con, host.clone()),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingConnection(Tx2EpConnection { con, url }) => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con, host.clone()),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::ConnectionClosed(Tx2EpConnectionClosed { con, url, .. }) => {
                        if evt_send
                            .send(MetaNetEvt::Disconnected {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con, host.clone()),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingRequest(Tx2EpIncomingRequest {
                        con,
                        url,
                        data,
                        respond,
                    }) => {
                        let timeout = tuning_params.implicit_timeout();
                        if evt_send
                            .send(MetaNetEvt::Request {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con, host.clone()),
                                data,
                                respond: Box::new(move |data| {
                                    let out: RespondFut = Box::pin(async move {
                                        let _ = respond.respond(data, timeout).await;
                                    });
                                    out
                                }),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::IncomingNotify(Tx2EpIncomingNotify { con, url, data, .. }) => {
                        if evt_send
                            .send(MetaNetEvt::Notify {
                                remote_url: url.to_string(),
                                con: MetaNetCon::Tx2(con, host.clone()),
                                data,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Tx2EpEvent::Error(_) | Tx2EpEvent::Tick | Tx2EpEvent::EndpointClosed => (),
                }
            }
        });

        Ok((MetaNet::Tx2(ep_hnd, return_host), evt_recv))
    }

    /// Construct abstraction with tx5 backend.
    #[cfg(feature = "tx5")]
    pub async fn new_tx5(
        tuning_params: KitsuneP2pTuningParams,
        host: HostApi,
        kitsune_internal_sender: ghost_actor::GhostSender<crate::spawn::Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
        signal_url: String,
    ) -> KitsuneP2pResult<(Self, MetaNetEvtRecv)> {
        let (mut evt_send, evt_recv) =
            futures::channel::mpsc::channel(tuning_params.concurrent_limit_per_thread);

        let mut tx5_config = tx5::DefConfig::default()
            .with_max_send_bytes(tuning_params.tx5_max_send_bytes)
            .with_max_recv_bytes(tuning_params.tx5_max_recv_bytes)
            .with_max_conn_count(tuning_params.tx5_max_conn_count)
            .with_max_conn_init(tuning_params.tx5_max_conn_init())
            .with_conn_preflight(move |_, _| {
                let i_s = kitsune_internal_sender.clone();

                Box::pin(async move {
                    match i_s.get_all_local_joined_agent_infos().await {
                        Ok(agent_list) => Ok(wire::Wire::peer_unsolicited(agent_list)
                            .encode_vec()
                            .ok()
                            .map(|v| v.into())),
                        Err(err) => {
                            tracing::warn!(?err, "error getting local peer list");
                            Ok(None)
                        }
                    }
                })
            })
            .with_conn_validate(move |_, _, maybe_data| {
                let e_s = evt_sender.clone();
                Box::pin(async move {
                    match maybe_data.map(|data| wire::Wire::decode_ref(&data)) {
                        Some(Ok((
                            _,
                            wire::Wire::PeerUnsolicited(wire::PeerUnsolicited { peer_list }),
                        ))) => {
                            // @todo This loop only exists because we have to put a
                            // space on PutAgentInfoSignedEvt, if the internal peer
                            // space was used instead we could do this in a single
                            // event with the whole list.
                            for peer in peer_list {
                                if let Err(err) = e_s
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        space: peer.space.clone(),
                                        peer_data: vec![peer.clone()],
                                    })
                                    .await
                                {
                                    tracing::warn!(
                                        ?err,
                                        "error processing incoming agent info unsolicited"
                                    );
                                }
                            }
                        }
                        Some(Err(err)) => tracing::warn!(?err, "error decoding connection peers"),
                        _ => {}
                    }
                    Ok(())
                })
            });

        tracing::info!(/*?tx5_config,*/ "meta net startup tx5");

        if let Some(lair_client) = host.lair_client() {
            tx5_config.set_lair_client(lair_client);
        }

        if let Some(lair_tag) = host.lair_tag() {
            tx5_config.set_lair_tag(lair_tag);
        }

        if let Err(err) = (tx5::deps::tx5_core::Tx5InitConfig {
            ephemeral_udp_port_min: tuning_params.tx5_min_ephemeral_udp_port,
            ephemeral_udp_port_max: tuning_params.tx5_max_ephemeral_udp_port,
        })
        .set_as_global_default()
        {
            tracing::warn!(?err, "Tx5InitConfig failed, you must be running multiple conductors in the same process. Be aware they will all share whichever Tx5InitConfig was first to be registered.");
        }
        let (ep_hnd, mut ep_evt) = tx5::Ep::with_config(tx5_config).await?;

        let cli_url = ep_hnd.listen(tx5::Tx5Url::new(&signal_url)?).await?;
        tracing::info!(%cli_url, "tx5 listening at url");

        let res_store = Arc::new(Mutex::new(HashMap::new()));

        let ep_hnd2 = ep_hnd.clone();
        let res_store2 = res_store.clone();
        let tuning_params2 = tuning_params.clone();
        let spawn_host = host.clone();
        tokio::task::spawn(async move {
            while let Some(evt) = ep_evt.recv().await {
                let evt = match evt {
                    Ok(evt) => evt,
                    Err(err) => {
                        tracing::error!(?err, "tx5 err event");
                        continue;
                    }
                };

                match evt {
                    tx5::EpEvt::Connected { rem_cli_url } => {
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: rem_cli_url.to_string(),
                                con: MetaNetCon::Tx5 {
                                    host: spawn_host.clone(),
                                    ep: ep_hnd2.clone(),
                                    rem_url: rem_cli_url,
                                    res: res_store2.clone(),
                                    tun: tuning_params2.clone(),
                                },
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tx5::EpEvt::Disconnected { rem_cli_url } => {
                        if evt_send
                            .send(MetaNetEvt::Disconnected {
                                remote_url: rem_cli_url.to_string(),
                                con: MetaNetCon::Tx5 {
                                    host: spawn_host.clone(),
                                    ep: ep_hnd2.clone(),
                                    rem_url: rem_cli_url,
                                    res: res_store2.clone(),
                                    tun: tuning_params2.clone(),
                                },
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tx5::EpEvt::Data {
                        rem_cli_url,
                        data,
                        permit,
                    } => {
                        tracing::trace!(%rem_cli_url, byte_count=?data.remaining(), "received bytes");

                        match WireWrap::decode(&mut bytes::Buf::reader(data)) {
                            Ok(WireWrap::Notify(Notify { msg_id, data })) => {
                                match wire::Wire::decode_ref(&data) {
                                    Ok((_, data)) => {
                                        tracing::trace!(%msg_id, ?data, "received notify");
                                        if evt_send
                                            .send(MetaNetEvt::Notify {
                                                remote_url: rem_cli_url.to_string(),
                                                con: MetaNetCon::Tx5 {
                                                    host: spawn_host.clone(),
                                                    ep: ep_hnd2.clone(),
                                                    rem_url: rem_cli_url,
                                                    res: res_store2.clone(),
                                                    tun: tuning_params2.clone(),
                                                },
                                                data,
                                            })
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!(?err, "decoding error");
                                        // TODO - drop connection??
                                    }
                                }
                            }
                            Ok(WireWrap::Request(Request { msg_id, data })) => {
                                match wire::Wire::decode_ref(&data) {
                                    Ok((_, data)) => {
                                        let ep_hnd = ep_hnd2.clone();
                                        let rem_cli_url2 = rem_cli_url.clone();
                                        let respond: Respond = Box::new(move |data| {
                                            let out: RespondFut = Box::pin(async move {
                                                let wire = match data.encode_vec() {
                                                    Ok(wire) => wire,
                                                    Err(_) => return,
                                                };
                                                let wrap =
                                                    WireWrap::response(msg_id, WireData(wire));
                                                let data = match wrap.encode_vec() {
                                                    Ok(data) => data,
                                                    Err(_) => return,
                                                };
                                                let _ = ep_hnd
                                                    .send(rem_cli_url2, data.as_slice())
                                                    .await;
                                            });
                                            out
                                        });
                                        if evt_send
                                            .send(MetaNetEvt::Request {
                                                remote_url: rem_cli_url.to_string(),
                                                con: MetaNetCon::Tx5 {
                                                    host: spawn_host.clone(),
                                                    ep: ep_hnd2.clone(),
                                                    rem_url: rem_cli_url,
                                                    res: res_store2.clone(),
                                                    tun: tuning_params2.clone(),
                                                },
                                                data,
                                                respond,
                                            })
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        tracing::error!(?err, "decoding error");
                                        // TODO - drop connection??
                                    }
                                }
                            }
                            Ok(WireWrap::Response(Response { msg_id, data })) => {
                                if let Some(s) = res_store2.lock().remove(&msg_id) {
                                    match wire::Wire::decode_ref(&data) {
                                        Ok((_, data)) => {
                                            let _ = s.send(data);
                                        }
                                        Err(err) => {
                                            tracing::error!(?err, "decoding error");
                                            // TODO - drop connection??
                                        }
                                    }
                                } else {
                                    tracing::debug!(%msg_id, "response mismatch");
                                }
                            }
                            Err(err) => {
                                tracing::error!(?err, "decoding error");
                                // TODO - drop connection??
                                continue;
                            }
                        }
                    }
                    tx5::EpEvt::Demo { rem_cli_url: _ } => (),
                }
            }
        });

        Ok((
            MetaNet::Tx5 {
                host,
                ep: ep_hnd,
                url: cli_url,
                res: res_store,
                tun: tuning_params,
            },
            evt_recv,
        ))
    }

    pub fn local_addr(&self) -> KitsuneResult<String> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep, _) = self {
                return ep.local_addr().map(|s| s.to_string());
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNet::Tx5 { url, .. } = self {
                return Ok(url.to_string());
            }
        }

        panic!("invalid features");
    }

    pub fn local_id(&self) -> Arc<[u8; 32]> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep, _) = self {
                return ep.local_cert().into();
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNet::Tx5 { url, .. } = self {
                if let Some(id) = url.id() {
                    return Arc::new(id.0);
                }
            }
        }

        panic!("invalid features");
    }

    pub async fn broadcast(
        &self,
        payload: &wire::Wire,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        let msg_id = next_msg_id();

        #[cfg(feature = "tx2")]
        {
            tracing::debug!("broadcast on tx2");
            return Ok(());
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNet::Tx5 { ep, .. } = self {
                let wire = payload.encode_vec().map_err(KitsuneError::other)?;
                let wrap = WireWrap::notify(msg_id, WireData(wire));

                let data = wrap.encode_vec().map_err(KitsuneError::other)?;
                ep.broadcast(data.as_slice())
                    .await
                    .map_err(KitsuneError::other)?;
                return Ok(());
            }
        }

        Err("invalid features".into())
    }

    pub async fn close(&self, code: u32, reason: &str) {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep, _) = self {
                ep.close(code, reason).await;
                return;
            }
        }

        // TODO - currently no way to shutdown tx5
    }

    pub async fn get_connection(
        &self,
        remote_url: String,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<MetaNetCon> {
        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep, host) = self {
                let con = ep.get_connection(remote_url, timeout).await?;
                return Ok(MetaNetCon::Tx2(con, host.clone()));
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNet::Tx5 {
                host, ep, res, tun, ..
            } = self
            {
                return Ok(MetaNetCon::Tx5 {
                    host: host.clone(),
                    ep: ep.clone(),
                    rem_url: tx5::Tx5Url::new(remote_url).map_err(KitsuneError::other)?,
                    res: res.clone(),
                    tun: tun.clone(),
                });
            }
        }

        Err("invalid features".into())
    }

    pub fn dump_network_stats(
        &self,
    ) -> impl std::future::Future<Output = KitsuneResult<serde_json::Value>> + 'static + Send {
        use futures::FutureExt;

        #[cfg(feature = "tx2")]
        {
            if let MetaNet::Tx2(ep, _) = self {
                let mut res = ep.debug();
                if let Some(map) = res.as_object_mut() {
                    map.insert("backend".into(), "tx2-quic".into());
                }
                return async move { Ok(res) }.boxed();
            }
        }

        #[cfg(feature = "tx5")]
        {
            if let MetaNet::Tx5 { ep, .. } = self {
                let fut = ep.get_stats();
                return async move { fut.await.map_err(KitsuneError::other) }.boxed();
            }
        }

        async move { Err("invalid features".into()) }.boxed()
    }
}
