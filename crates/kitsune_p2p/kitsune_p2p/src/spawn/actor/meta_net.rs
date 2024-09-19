// because of feature flipping
#![allow(dead_code)]
#![allow(irrefutable_let_patterns)]
#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unreachable_patterns)]
#![allow(clippy::needless_return)]
//! Networking abstraction to handle feature flipping.

use crate::wire::WireData;
use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use kitsune_p2p_types::tx_utils::TxUrl;

use crate::spawn::actor::InternalSender;
use crate::spawn::KitsuneP2pEvent;
use crate::spawn::PutAgentInfoSignedEvt;
use crate::types::event::KitsuneP2pEventSender;
use kitsune_p2p_block::BlockTargetId;
use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::agent_info::AgentInfoSigned;
use kitsune_p2p_types::codec::Codec;
use kitsune_p2p_types::config::KitsuneP2pConfig;
use kitsune_p2p_types::config::KitsuneP2pTuningParams;
use kitsune_p2p_types::*;
use opentelemetry_api::metrics::Histogram;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tx5::PeerUrl;

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

kitsune_p2p_types::write_codec_enum! {
    /// Preflight data for tx5.
    /// Since this is all about compatibility, the codec itself contains versioned payloads,
    /// in case the preflight check needs to evolve over time.
    codec PreflightData {
        /// Version 0
        V0(0) {
            /// Kitsune protocol version which is bumped at every breaking change
            kitsune_protocol_version.0: u16,
            /// Our local peer info
            peer_list.1: Vec<AgentInfoSigned>,
            /// Data provided by the host, which must match across nodes in order
            /// for preflight to succeed
            user_data.2: Vec<u8>,
        },
    }
}

/// Host-defined data used to implement custom connection preflight checks.
///
/// The `bytes` are sent with every preflight, and the `comparator` is used to validate
/// the bytes sent by the remote peer. If the comparator returns an Err, the preflight
/// fails and no connection is made.
///
/// The string returned in the Err is logged from kitsune to indicate the point of failure.
pub struct PreflightUserData {
    /// The bytes to send with every preflight.
    pub bytes: Vec<u8>,
    /// The comparator function to use to validate the bytes sent by the remote peer.
    ///
    /// Typically this will be a closure that captures the bytes sent, so that the two values can
    /// be compared.
    #[allow(clippy::type_complexity)]
    pub comparator: Box<dyn Fn(&tx5::PeerUrl, &[u8]) -> Result<(), String> + Send + Sync + 'static>,
}

impl Default for PreflightUserData {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            comparator: Box::new(|_, _| Ok(())),
        }
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
    /// This node has a new address at which it can be reached.
    NewAddress {
        /// The new address at which this node can be reached.
        local_url: String,
    },

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

impl std::fmt::Debug for MetaNetEvt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewAddress { local_url } => f
                .debug_struct("NewAddress")
                .field("local_url", local_url)
                .finish(),
            Self::Connected { remote_url, .. } => f
                .debug_struct("Connected")
                .field("remote_url", remote_url)
                .finish(),
            Self::Disconnected { remote_url, .. } => f
                .debug_struct("Disconnected")
                .field("remote_url", remote_url)
                .finish(),
            Self::Request {
                remote_url, data, ..
            } => f
                .debug_struct("Request")
                .field("remote_url", remote_url)
                .field("data", data)
                .finish(),
            Self::Notify {
                remote_url, data, ..
            } => f
                .debug_struct("Notify")
                .field("remote_url", remote_url)
                .field("data", data)
                .finish(),
        }
    }
}

impl MetaNetEvt {
    pub fn maybe_con(&self) -> Option<&MetaNetCon> {
        match self {
            MetaNetEvt::NewAddress { .. } => None,
            MetaNetEvt::Connected { con, .. }
            | MetaNetEvt::Disconnected { con, .. }
            | MetaNetEvt::Request { con, .. }
            | MetaNetEvt::Notify { con, .. } => Some(con),
        }
    }

    pub fn maybe_space(&self) -> Option<Arc<KitsuneSpace>> {
        match self {
            MetaNetEvt::Request { data, .. } | MetaNetEvt::Notify { data, .. } => {
                data.maybe_space()
            }
            MetaNetEvt::NewAddress { .. }
            | MetaNetEvt::Connected { .. }
            | MetaNetEvt::Disconnected { .. } => None,
        }
    }
}

pub enum MetaNetAuth {
    Authorized,
    UnauthorizedIgnore,
    UnauthorizedDisconnect,
}

async fn node_is_authorized(host: &HostApi, node_id: NodeCert, now: Timestamp) -> MetaNetAuth {
    match host.is_blocked(BlockTargetId::Node(node_id), now).await {
        Ok(true) => MetaNetAuth::UnauthorizedDisconnect,
        Ok(false) => MetaNetAuth::Authorized,
        Err(_) => MetaNetAuth::UnauthorizedIgnore,
    }
}

pub async fn nodespace_is_authorized(
    host: &HostApi,
    node_id: NodeCert,
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
    rem_id: tx5::PubKey,
    is_error: bool,
    byte_count: u64,
    start_time: std::time::Instant,
}

impl MetricSendGuard {
    pub fn new(rem_id: tx5::PubKey, byte_count: u64) -> Self {
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
                opentelemetry_api::KeyValue::new("remote_id", format!("{:?}", self.rem_id)),
                opentelemetry_api::KeyValue::new("is_error", self.is_error),
            ],
        );
        crate::metrics::METRIC_MSG_OUT_TIME.record(
            self.start_time.elapsed().as_secs_f64(),
            &[
                opentelemetry_api::KeyValue::new("remote_id", format!("{:?}", self.rem_id)),
                opentelemetry_api::KeyValue::new("is_error", self.is_error),
            ],
        );
    }
}

#[derive(Debug, Clone)]
pub enum MetaNetCon {
    Tx5 {
        host: HostApiLegacy,
        ep: Arc<tx5::Endpoint>,
        rem_url: tx5::PeerUrl,
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
            (MetaNetCon::Tx5 { ep: a, .. }, MetaNetCon::Tx5 { ep: b, .. }) => Arc::ptr_eq(a, b),
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

        {
            if let MetaNetCon::Tx5 {
                ep, rem_url, tun, ..
            } = self
            {
                ep.close(rem_url);
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

        {
            // NOTE - tx5 connections are never exactly "closed"
            //        since it's more of a message queue...
            return false;
        }
    }

    async fn wire_is_authorized(&self, payload: &wire::Wire, now: Timestamp) -> MetaNetAuth {
        match self {
            MetaNetCon::Tx5 { host, .. } => {
                nodespace_is_authorized(host, self.peer_id(), payload.maybe_space(), now).await
            }
            #[cfg(test)]
            MetaNetCon::Test { .. } => MetaNetAuth::Authorized,
        }
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
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

                    {
                        if let MetaNetCon::Tx5 { ep, rem_url, .. } = self {
                            let wire = payload.encode_vec().map_err(KitsuneError::other)?;
                            let wrap = WireWrap::notify(msg_id, WireData(wire));

                            let data = wrap.encode_vec().map_err(KitsuneError::other)?;

                            let mut metric_guard =
                                MetricSendGuard::new(rem_url.pub_key().clone(), data.len() as u64);

                            ep.send(rem_url.clone(), data)
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
                                MetricSendGuard::new(rem_url.pub_key().clone(), data.len() as u64);

                            ep.send(rem_url.clone(), data)
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

    pub fn peer_id(&self) -> NodeCert {
        #[cfg(test)]
        {
            if let MetaNetCon::Test { state } = self {
                return state.read().id();
            }
        }

        {
            if let MetaNetCon::Tx5 { rem_url, .. } = self {
                return rem_url.pub_key().0.clone().into();
            }
        }

        panic!("invalid features");
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct MetaNetConTest {
    pub id: NodeCert,
    pub closed: bool,

    pub notify_succeed: bool,
    pub notify_call_count: usize,
}

#[cfg(test)]
impl Default for MetaNetConTest {
    fn default() -> Self {
        Self {
            id: NodeCert::from(Arc::new([0; 32])),
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
            id: NodeCert::from(Arc::new(vec![id; 32].try_into().unwrap())),
            ..Default::default()
        }
    }

    pub fn id(&self) -> NodeCert {
        self.id.clone()
    }
}

/// Networking abstraction to handle feature flipping.
#[derive(Clone)]
pub enum MetaNet {
    /// Tx5 Abstraction
    Tx5 {
        host: HostApiLegacy,
        ep: Arc<tx5::Endpoint>,
        res: ResStore,
        tun: KitsuneP2pTuningParams,
    },
}

impl MetaNet {
    /// Construct abstraction with tx5 backend.
    pub async fn new_tx5(
        tuning_params: KitsuneP2pTuningParams,
        host: HostApiLegacy,
        kitsune_internal_sender: ghost_actor::GhostSender<crate::spawn::Internal>,
        signal_url: String,
        webrtc_config: String,
        preflight_user_data: PreflightUserData,
    ) -> KitsuneP2pResult<(Self, MetaNetEvtRecv, Option<String>)> {
        use kitsune_p2p_types::codec::{rmp_decode, rmp_encode};

        let (mut evt_send, evt_recv) =
            futures::channel::mpsc::channel(tuning_params.concurrent_limit_per_thread);

        let PreflightUserData {
            bytes: user_data_sent,
            comparator: user_data_cmp,
        } = preflight_user_data;

        let evt_sender = host.legacy.clone();
        let tx5_config = tx5::Config {
            // TODO: once we implement local discovery, we should only
            //       allow plain text over local connections. But for now
            //       we cannot distinguish, so in order for run-local-services
            //       to work, we need to allow plain text on ALL connections
            signal_allow_plain_text: true,
            initial_webrtc_config: webrtc_config,
            connection_count_max: tuning_params.tx5_connection_count_max,
            send_buffer_bytes_max: tuning_params.tx5_send_buffer_bytes_max,
            recv_buffer_bytes_max: tuning_params.tx5_recv_buffer_bytes_max,
            incoming_message_bytes_max: tuning_params.tx5_incoming_message_bytes_max,
            message_size_max: tuning_params.tx5_message_size_max,
            internal_event_channel_size: tuning_params.tx5_internal_event_channel_size,
            timeout: std::time::Duration::from_secs(tuning_params.tx5_timeout_s as u64),
            backoff_start: std::time::Duration::from_secs(tuning_params.tx5_backoff_start_s as u64),
            backoff_max: std::time::Duration::from_secs(tuning_params.tx5_backoff_max_s as u64),
            preflight: Some((
                Arc::new(move |_| {
                    let i_s = kitsune_internal_sender.clone();
                    let user_data_sent = user_data_sent.clone();

                    Box::pin(async move {
                        let agent_list = i_s
                            .get_all_local_joined_agent_infos()
                            .await
                            .unwrap_or_default();
                        PreflightData::v0(KITSUNE_PROTOCOL_VERSION, agent_list, user_data_sent)
                            .encode_vec()
                    })
                }),
                Arc::new(move |url, data| {
                    let e_s = evt_sender.clone();
                    let url = url.clone();
                    match PreflightData::decode_ref(&data) {
                        Ok((
                            _,
                            PreflightData::V0(V0 {
                                kitsune_protocol_version,
                                peer_list,
                                user_data: user_data_bytes_received,
                            }),
                        )) => {
                            if kitsune_protocol_version != KITSUNE_PROTOCOL_VERSION {
                                tracing::warn!(
                                    ?url,
                                    "kitsune protocol version mismatch: ours = {}, theirs = {}",
                                    KITSUNE_PROTOCOL_VERSION,
                                    kitsune_protocol_version,
                                );
                                return box_fut_plain(Err(std::io::Error::other(
                                    "kitsune protocol version mismatch",
                                )));
                            }

                            if let Err(reason) = user_data_cmp(&url, &user_data_bytes_received) {
                                tracing::warn!(?url, "tx5 preflight user_data mismatch");
                                return box_fut_plain(Err(std::io::Error::other(
                                    "tx5 preflight user_data mismatch",
                                )));
                            }
                            Box::pin(async move {
                                if let Err(err) = e_s
                                    .put_agent_info_signed(PutAgentInfoSignedEvt {
                                        peer_data: peer_list,
                                    })
                                    .await
                                {
                                    tracing::warn!(
                                        ?err,
                                        "error processing incoming agent info unsolicited"
                                    );
                                }
                                Ok(())
                            })
                        }
                        Err(err) => {
                            tracing::warn!(?err, ?url, "Could not decode PreflightData");
                            box_fut_plain(Err(std::io::Error::other(
                                "Could not decode PreflightData",
                            )))
                        }
                        _ => box_fut_plain(Err(std::io::Error::other("Unexpected wire message"))),
                    }
                }),
            )),
            //..Default::default()
        };

        tracing::info!(?tx5_config, "meta net startup tx5");

        if let Err(err) = (tx5::Tx5InitConfig {
            tracing_enabled: tuning_params.tx5_backend_tracing_enabled,
            ephemeral_udp_port_min: tuning_params.tx5_min_ephemeral_udp_port,
            ephemeral_udp_port_max: tuning_params.tx5_max_ephemeral_udp_port,
            ..Default::default()
        })
        .set_as_global_default()
        {
            tracing::warn!(?err, "Tx5InitConfig failed, you must be running multiple conductors in the same process. Be aware they will all share whichever Tx5InitConfig was first to be registered.");
        }
        let (ep_hnd, mut ep_evt) = tx5::Endpoint::new(Arc::new(tx5_config));
        let ep_hnd = Arc::new(ep_hnd);

        let maybe_peer_url = ep_hnd
            .listen(tx5::SigUrl::parse(&signal_url)?)
            .await
            .map(|p| p.to_string());

        let res_store = Arc::new(Mutex::new(HashMap::new()));

        let ep_hnd2 = ep_hnd.clone();
        let res_store2 = res_store.clone();
        let tuning_params2 = tuning_params.clone();
        let spawn_host = host.clone();
        tokio::task::spawn(async move {
            while let Some(evt) = ep_evt.recv().await {
                match evt {
                    tx5::EndpointEvent::ListeningAddressOpen { local_url } => {
                        tracing::info!(%local_url, "listening open");
                        if evt_send
                            .send(MetaNetEvt::NewAddress {
                                local_url: local_url.to_string(),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tx5::EndpointEvent::ListeningAddressClosed { local_url } => {
                        tracing::info!(%local_url, "listening closed");
                        // TODO: publish close agent_info
                    }
                    tx5::EndpointEvent::Connected { peer_url } => {
                        tracing::debug!(%peer_url, "peer connected");
                        if evt_send
                            .send(MetaNetEvt::Connected {
                                remote_url: peer_url.to_string(),
                                con: MetaNetCon::Tx5 {
                                    host: spawn_host.clone(),
                                    ep: ep_hnd2.clone(),
                                    rem_url: peer_url,
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
                    tx5::EndpointEvent::Disconnected { peer_url } => {
                        tracing::debug!(%peer_url, "peer disconnected");
                        if evt_send
                            .send(MetaNetEvt::Disconnected {
                                remote_url: peer_url.to_string(),
                                con: MetaNetCon::Tx5 {
                                    host: spawn_host.clone(),
                                    ep: ep_hnd2.clone(),
                                    rem_url: peer_url,
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
                    tx5::EndpointEvent::Message { peer_url, message } => {
                        tracing::trace!(%peer_url, byte_count=?message.len(), "received bytes");

                        let mut message = std::io::Cursor::new(&message);
                        match WireWrap::decode(&mut message) {
                            Ok(WireWrap::Notify(Notify { msg_id, data })) => {
                                match wire::Wire::decode_ref(&data) {
                                    Ok((_, data)) => {
                                        tracing::trace!(%msg_id, ?data, "received notify");
                                        if evt_send
                                            .send(MetaNetEvt::Notify {
                                                remote_url: peer_url.to_string(),
                                                con: MetaNetCon::Tx5 {
                                                    host: spawn_host.clone(),
                                                    ep: ep_hnd2.clone(),
                                                    rem_url: peer_url,
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
                                        ep_hnd2.close(&peer_url);
                                    }
                                }
                            }
                            Ok(WireWrap::Request(Request { msg_id, data })) => {
                                match wire::Wire::decode_ref(&data) {
                                    Ok((_, data)) => {
                                        let ep_hnd = ep_hnd2.clone();
                                        let peer_url2 = peer_url.clone();
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
                                                let _ = ep_hnd.send(peer_url2, data).await;
                                            });
                                            out
                                        });
                                        if evt_send
                                            .send(MetaNetEvt::Request {
                                                remote_url: peer_url.to_string(),
                                                con: MetaNetCon::Tx5 {
                                                    host: spawn_host.clone(),
                                                    ep: ep_hnd2.clone(),
                                                    rem_url: peer_url,
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
                                        ep_hnd2.close(&peer_url);
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
                                            ep_hnd2.close(&peer_url);
                                        }
                                    }
                                } else {
                                    tracing::debug!(%msg_id, "response mismatch");
                                }
                            }
                            Err(err) => {
                                tracing::error!(?err, "decoding error");
                                ep_hnd2.close(&peer_url);
                                continue;
                            }
                        }
                    }
                }
            }
        });

        Ok((
            MetaNet::Tx5 {
                host,
                ep: ep_hnd,
                res: res_store,
                tun: tuning_params,
            },
            evt_recv,
            maybe_peer_url,
        ))
    }


    pub async fn broadcast(
        &self,
        payload: &wire::Wire,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        let msg_id = next_msg_id();

        {
            if let MetaNet::Tx5 { ep, .. } = self {
                let wire = payload.encode_vec().map_err(KitsuneError::other)?;
                let wrap = WireWrap::notify(msg_id, WireData(wire));

                let data = wrap.encode_vec().map_err(KitsuneError::other)?;
                ep.broadcast(data.as_slice()).await;
                return Ok(());
            }
        }

        Err("invalid features".into())
    }

    pub async fn close(&self, code: u32, reason: &str) {

        // TODO - currently no way to shutdown tx5
    }

    pub fn close_peer_con(&self, peer_url: TxUrl) -> KitsuneResult<()> {
        // Not supported for tx2

                {
            // Even if tx5 is enabled, check that the peer_url is a ws or wss url to the signal server
            if peer_url.scheme() == "ws" || peer_url.scheme() == "wss" {
                if let MetaNet::Tx5 { ep, .. } = self {
                    let peer_url =
                        PeerUrl::parse(peer_url.to_string()).map_err(KitsuneError::other)?;
                    ep.close(&peer_url);
                }
            }
        }

        Ok(())
    }

    pub async fn get_connection(
        &self,
        remote_url: String,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<MetaNetCon> {
        {
            if let MetaNet::Tx5 {
                host, ep, res, tun, ..
            } = self
            {
                return Ok(MetaNetCon::Tx5 {
                    host: host.clone(),
                    ep: ep.clone(),
                    rem_url: tx5::PeerUrl::parse(remote_url).map_err(KitsuneError::other)?,
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

        {
            if let MetaNet::Tx5 { ep, .. } = self {
                let ep = ep.clone();
                let stats = ep.get_stats();
                return async move {
                    serde_json::from_str(
                        &serde_json::to_string(&stats).map_err(KitsuneError::other)?,
                    )
                    .map_err(KitsuneError::other)
                }
                .boxed();
            }
        }

        async move { Err("invalid features".into()) }.boxed()
    }
}

#[cfg(test)]
mod tests;
