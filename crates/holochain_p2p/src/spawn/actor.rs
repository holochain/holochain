#![allow(clippy::too_many_arguments)]

use crate::metrics::create_p2p_request_duration_metric;
use crate::*;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::sql::sql_peer_meta_store;
use holochain_state::prelude::named_params;
use kitsune2_api::*;
use kitsune2_core::get_remote_agents_near_location;
use std::collections::HashMap;
use std::sync::{Mutex, Weak};
use std::time::Duration;
use tokio::task::AbortHandle;

/// Hard-code for now.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

macro_rules! timing_trace {
    ($netaudit:literal, $code:block $($rest:tt)*) => {{
        let __start = std::time::Instant::now();
        let __out = $code;
        Box::pin(async move {
            let __out = __out.await;
            let __elapsed_s = __start.elapsed().as_secs_f64();
            if __elapsed_s >= 5.0 {
                if $netaudit {
                    tracing::warn!( target: "NETAUDIT", m = "holochain_p2p", elapsed_s = %__elapsed_s $($rest)* );
                } else {
                    tracing::warn!( elapsed_s = %__elapsed_s $($rest)* );
                }
            } else {
                if $netaudit {
                    tracing::trace!( target: "NETAUDIT", m = "holochain_p2p", elapsed_s = %__elapsed_s $($rest)* );
                } else {
                    tracing::trace!( elapsed_s = %__elapsed_s $($rest)* );
                }
            }
            __out
        })
    }};
}

/// Evt wrapper to allow timing traces.
#[derive(Clone, Debug)]
pub struct WrapEvtSender(pub event::DynHcP2pHandler);

impl event::HcP2pHandler for WrapEvtSender {
    fn handle_call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        let byte_count = zome_call_params_serialized.0.len();
        timing_trace!(
            true,
            {
                self.0.handle_call_remote(
                    dna_hash,
                    to_agent,
                    zome_call_params_serialized,
                    signature,
                )
            },
            byte_count,
            a = "recv_call_remote",
        )
    }

    fn handle_publish(
        &self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        let op_count = ops.len();
        timing_trace!(
            true,
            {
                self.0.handle_publish(dna_hash, request_validation_receipt, ops)
            }, %op_count, a = "recv_publish")
    }

    fn handle_get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        timing_trace!(
            true,
            { self.0.handle_get(dna_hash, to_agent, dht_hash) },
            a = "recv_get",
        )
    }

    fn handle_get_meta(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        timing_trace!(
            true,
            {
                self.0
                    .handle_get_meta(dna_hash, to_agent, dht_hash, options)
            },
            a = "recv_get_meta",
        )
    }

    fn handle_get_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        timing_trace!(
            true,
            {
                self.0
                    .handle_get_links(dna_hash, to_agent, link_key, options)
            },
            a = "recv_get_links",
        )
    }

    fn handle_count_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        timing_trace!(
            true,
            { self.0.handle_count_links(dna_hash, to_agent, query) },
            a = "recv_count_links"
        )
    }

    fn handle_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<AgentActivityResponse>> {
        timing_trace!(
            true,
            {
                self.0
                    .handle_get_agent_activity(dna_hash, to_agent, agent, query, options)
            },
            a = "recv_get_agent_activity",
        )
    }

    fn handle_must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<MustGetAgentActivityResponse>> {
        timing_trace!(
            true,
            {
                self.0
                    .handle_must_get_agent_activity(dna_hash, to_agent, agent, filter)
            },
            a = "recv_must_get_agent_activity",
        )
    }

    fn handle_validation_receipts_received(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        timing_trace!(
            false,
            {
                self.0
                    .handle_validation_receipts_received(dna_hash, to_agent, receipts)
            },
            a = "recv_validation_receipt_received",
        )
    }

    fn handle_publish_countersign(
        &self,
        dna_hash: DnaHash,
        op: holochain_types::dht_op::ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        timing_trace!(
            true,
            { self.0.handle_publish_countersign(dna_hash, op) },
            a = "recv_publish_countersign"
        )
    }

    fn handle_countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        timing_trace!(
            false,
            {
                self.0
                    .handle_countersigning_session_negotiation(dna_hash, to_agent, message)
            },
            a = "recv_countersigning_session_negotiation"
        )
    }
}

type Respond = tokio::sync::oneshot::Sender<crate::wire::WireMessage>;

struct Pending {
    this: Weak<Mutex<Self>>,
    map: HashMap<u64, Respond>,
}

impl Pending {
    fn register(&mut self, msg_id: u64, resp: Respond) {
        if let Some(this) = self.this.upgrade() {
            self.map.insert(msg_id, resp);
            tokio::task::spawn(async move {
                tokio::time::sleep(REQUEST_TIMEOUT).await;
                let _ = this.lock().unwrap().respond(msg_id);
            });
        }
    }

    fn respond(&mut self, msg_id: u64) -> Option<Respond> {
        self.map.remove(&msg_id)
    }
}

pub(crate) struct HolochainP2pActor {
    this: Weak<Self>,
    target_arc_factor: u32,
    compat: NetworkCompatParams,
    preflight: Arc<Mutex<bytes::Bytes>>,
    evt_sender: Arc<std::sync::OnceLock<WrapEvtSender>>,
    lair_client: holochain_keystore::MetaLairClient,
    kitsune: DynKitsune,
    pending: Arc<Mutex<Pending>>,
    request_duration_metric: metrics::P2pRequestDurationMetric,
    pruning_task_abort_handle: AbortHandle,
}

impl std::fmt::Debug for HolochainP2pActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainP2pActor").finish()
    }
}

const EVT_REG_ERR: &str = "event handler not registered";

impl SpaceHandler for HolochainP2pActor {
    fn recv_notify(&self, from_peer: Url, space: SpaceId, data: bytes::Bytes) -> K2Result<()> {
        for msg in WireMessage::decode_batch(&data).map_err(|err| {
            K2Error::other_src("decode incoming holochain_p2p wire message batch", err)
        })? {
            // NOTE: spawning a task here could lead to memory issues
            //       in the case of DoS messaging, consider some kind
            //       of queue or semaphore.
            let from_peer = from_peer.clone();
            let space_id = space.clone();
            let evt_sender = self.evt_sender.clone();
            let kitsune = self.kitsune.clone();
            let pending = self.pending.clone();
            let this = self.this.clone();
            tokio::task::spawn(async move {
                use crate::event::HcP2pHandler;
                use crate::wire::WireMessage::*;
                match msg {
                    ErrorRes { msg_id, .. }
                    | CallRemoteRes { msg_id, .. }
                    | GetRes { msg_id, .. }
                    | GetMetaRes { msg_id, .. }
                    | GetLinksRes { msg_id, .. }
                    | CountLinksRes { msg_id, .. }
                    | GetAgentActivityRes { msg_id, .. }
                    | MustGetAgentActivityRes { msg_id, .. }
                    | SendValidationReceiptsRes { msg_id } => {
                        if let Some(resp) = pending.lock().unwrap().respond(msg_id) {
                            let _ = resp.send(msg);
                        }
                    }
                    CallRemoteReq {
                        msg_id,
                        to_agent,
                        zome_call_params_serialized,
                        signature,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_call_remote(
                                dna_hash,
                                to_agent,
                                zome_call_params_serialized,
                                signature,
                            )
                            .await
                        {
                            Ok(response) => CallRemoteRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };

                        if let Some(this) = this.upgrade() {
                            if let Err(err) = this
                                .send_notify_response(space_id, from_peer, msg_id, resp)
                                .await
                            {
                                tracing::debug!(?err, "Error sending call remote response");
                            }
                        } else {
                            tracing::debug!("HolochainP2pActor has been dropped");
                        }
                    }
                    GetReq {
                        msg_id,
                        to_agent,
                        dht_hash,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_get(dna_hash, to_agent, dht_hash)
                            .await
                        {
                            Ok(response) => GetRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending get response");
                        }
                    }
                    GetMetaReq {
                        msg_id,
                        to_agent,
                        dht_hash,
                        options,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_get_meta(dna_hash, to_agent, dht_hash, options)
                            .await
                        {
                            Ok(response) => GetMetaRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending get_meta response");
                        }
                    }
                    GetLinksReq {
                        msg_id,
                        to_agent,
                        link_key,
                        options,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_get_links(dna_hash, to_agent, link_key, options)
                            .await
                        {
                            Ok(response) => GetLinksRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending get_links response");
                        }
                    }
                    CountLinksReq {
                        msg_id,
                        to_agent,
                        query,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_count_links(dna_hash, to_agent, query)
                            .await
                        {
                            Ok(response) => CountLinksRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending count_links response");
                        }
                    }
                    GetAgentActivityReq {
                        msg_id,
                        to_agent,
                        agent,
                        query,
                        options,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_get_agent_activity(dna_hash, to_agent, agent, query, options)
                            .await
                        {
                            Ok(response) => GetAgentActivityRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending get_agent_activity response");
                        }
                    }
                    MustGetAgentActivityReq {
                        msg_id,
                        to_agent,
                        agent,
                        filter,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_must_get_agent_activity(dna_hash, to_agent, agent, filter)
                            .await
                        {
                            Ok(response) => MustGetAgentActivityRes { msg_id, response },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending must_get_agent_activity response");
                        }
                    }
                    SendValidationReceiptsReq {
                        msg_id,
                        to_agent,
                        receipts,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);

                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_validation_receipts_received(dna_hash, to_agent, receipts)
                            .await
                        {
                            Ok(_) => SendValidationReceiptsRes { msg_id },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(
                                ?err,
                                "Error sending send_validation_receipts response"
                            );
                        }
                    }
                    RemoteSignalEvt {
                        to_agent,
                        zome_call_params_serialized,
                        signature,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        // remote signals are fire-and-forget
                        // so it's safe to ignore the response
                        let _response = evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_call_remote(
                                dna_hash,
                                to_agent,
                                zome_call_params_serialized,
                                signature,
                            )
                            .await;
                    }
                    PublishCountersignEvt { op } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_publish_countersign(dna_hash, op)
                            .await?;
                    }
                    CountersigningSessionNegotiationEvt { to_agent, message } => {
                        let dna_hash = DnaHash::from_k2_space(&space_id);
                        evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_countersigning_session_negotiation(dna_hash, to_agent, message)
                            .await?;
                    }
                }
                HolochainP2pResult::Ok(())
            });
        }
        Ok(())
    }
}

impl kitsune2_api::KitsuneHandler for HolochainP2pActor {
    fn create_space(
        &self,
        _space: kitsune2_api::SpaceId,
    ) -> BoxFut<'_, kitsune2_api::K2Result<kitsune2_api::DynSpaceHandler>> {
        Box::pin(async move {
            let this: Weak<dyn kitsune2_api::SpaceHandler> = self.this.clone();
            if let Some(this) = this.upgrade() {
                Ok(this)
            } else {
                Err(kitsune2_api::K2Error::other(
                    "HolochainP2pActor instance has been dropped",
                ))
            }
        })
    }

    fn preflight_gather_outgoing(&self, _peer_url: Url) -> K2Result<bytes::Bytes> {
        Ok(self.preflight.lock().unwrap().clone())
    }

    fn preflight_validate_incoming(&self, _peer_url: Url, data: bytes::Bytes) -> K2Result<()> {
        // decode the preflight that the remote sent us
        let rem = crate::wire::WirePreflightMessage::decode(&data)
            .map_err(|err| K2Error::other_src("Invalid remote preflight", err))?;

        // if the compats don't match, reject the connection
        if rem.compat != self.compat {
            return Err(K2Error::other(format!(
                "Invalid remote preflight, wanted {:?}, got: {:?}",
                self.compat, rem.compat
            )));
        }

        let mut agents = Vec::with_capacity(rem.agents.len());

        // decode the agents inline, so we can reject the connection
        // if they sent us bad agent data
        for agent in rem.agents {
            agents.push(AgentInfoSigned::decode(
                &kitsune2_core::Ed25519Verifier,
                agent.as_bytes(),
            )?);
        }

        // if they sent us agents, spawn a task to insert them into the store
        if !agents.is_empty() {
            let kitsune = self.kitsune.clone();
            tokio::task::spawn(async move {
                for agent in agents {
                    let space = match kitsune.space_if_exists(agent.space.clone()).await {
                        None => continue,
                        Some(space) => space,
                    };
                    space.peer_store().insert(vec![agent]).await?;
                }
                K2Result::Ok(())
            });
        }

        Ok(())
    }
}

/// A wrapper for the default Bootstrap K2 module, so we can capture
/// the generated agent infos for publishing in the preflight.
#[derive(Debug)]
struct BootWrap {
    compat: NetworkCompatParams,
    preflight: Arc<std::sync::Mutex<bytes::Bytes>>,
    orig: kitsune2_api::DynBootstrap,
    cache: std::sync::Mutex<Vec<Arc<AgentInfoSigned>>>,
}

impl kitsune2_api::Bootstrap for BootWrap {
    fn put(&self, info: Arc<AgentInfoSigned>) {
        let Self {
            compat,
            preflight,
            orig,
            cache,
        } = self;

        let agents = {
            let mut cache = cache.lock().unwrap();

            // remove expired infos and previous infos that match the
            // one that was generated
            let now = kitsune2_api::Timestamp::now();
            cache.retain(|cache_info| {
                if cache_info.expires_at < now {
                    return false;
                }
                if cache_info.agent == info.agent && cache_info.space == info.space {
                    return false;
                }
                true
            });

            // add the one that was generated
            cache.push(info.clone());

            let mut agents = Vec::new();

            // string encoding
            for agent in cache.iter() {
                if let Ok(encoded) = agent.encode() {
                    agents.push(encoded);
                }
            }

            agents
        };

        // encode the preflight message and cache
        if let Ok(encoded) = (crate::wire::WirePreflightMessage {
            compat: compat.clone(),
            agents,
        })
        .encode()
        {
            *preflight.lock().unwrap() = encoded;
        }

        // don't forget to invoke the original bootstrap handler
        orig.put(info);
    }
}

/// This factory wraps the original bootstrap factory, generating
/// original bootstrap instances, and wrapping them with our wrapper.
#[derive(Debug)]
struct BootWrapFact {
    compat: NetworkCompatParams,
    preflight: Arc<std::sync::Mutex<bytes::Bytes>>,
    orig: kitsune2_api::DynBootstrapFactory,
}

impl kitsune2_api::BootstrapFactory for BootWrapFact {
    fn default_config(&self, config: &mut Config) -> K2Result<()> {
        self.orig.default_config(config)
    }

    fn validate_config(&self, config: &Config) -> K2Result<()> {
        self.orig.validate_config(config)
    }

    fn create(
        &self,
        builder: Arc<Builder>,
        peer_store: DynPeerStore,
        space: SpaceId,
    ) -> BoxFut<'static, K2Result<DynBootstrap>> {
        let compat = self.compat.clone();
        let preflight = self.preflight.clone();
        let orig_fut = self.orig.create(builder, peer_store, space);
        Box::pin(async move {
            let orig = orig_fut.await?;
            let out: DynBootstrap = Arc::new(BootWrap {
                compat,
                preflight,
                orig,
                cache: Default::default(),
            });
            Ok(out)
        })
    }
}

impl Drop for HolochainP2pActor {
    fn drop(&mut self) {
        self.pruning_task_abort_handle.abort();
    }
}

impl HolochainP2pActor {
    /// constructor
    pub async fn create(
        config: HolochainP2pConfig,
        lair_client: holochain_keystore::MetaLairClient,
    ) -> HolochainP2pResult<actor::DynHcP2p> {
        static K2_CONFIG: std::sync::Once = std::sync::Once::new();
        K2_CONFIG.call_once(|| {
            // Set up some kitsune2 specializations specific to holochain.

            // Kitsune2 by default just xors subsequent bytes of the hash
            // itself and treats that result as a LE u32.
            // Holochain, instead, first does a blake2b hash, and
            // then xors those bytes.
            kitsune2_api::Id::set_global_loc_callback(|bytes| {
                let hash = blake2b_simd::Params::new().hash_length(16).hash(bytes);
                let hash = hash.as_bytes();
                let mut out = [hash[0], hash[1], hash[2], hash[3]];
                for i in (4..16).step_by(4) {
                    out[0] ^= hash[i];
                    out[1] ^= hash[i + 1];
                    out[2] ^= hash[i + 2];
                    out[3] ^= hash[i + 3];
                }
                u32::from_le_bytes(out)
            });

            // Kitsune2 just displays the bytes as direct base64.
            // Holochain prepends some prefix bytes and appends the loc bytes.
            kitsune2_api::SpaceId::set_global_display_callback(|bytes, f| {
                write!(f, "{}", DnaHash::from_raw_32(bytes.to_vec()))
            });
            kitsune2_api::AgentId::set_global_display_callback(|bytes, f| {
                write!(f, "{}", AgentPubKey::from_raw_32(bytes.to_vec()))
            });
            kitsune2_api::OpId::set_global_display_callback(|bytes, f| {
                write!(f, "{}", DhtOpHash::from_raw_32(bytes.to_vec()))
            });
        });

        #[cfg(feature = "test_utils")]
        let mut builder = if config.k2_test_builder {
            let mut builder = kitsune2_core::default_test_builder();

            if config.disable_bootstrap {
                builder.bootstrap = Arc::new(test::NoopBootstrapFactory);
            }

            // Make it possible to disable the gossip module for testing.
            if !config.disable_gossip {
                // Still want the real gossip module to be used. The test builder comes with a stub
                // gossip module fur use in K2 testing.
                builder.gossip = kitsune2_gossip::K2GossipFactory::create();
            } else {
                tracing::info!("Running with gossip disabled");
            }

            if config.disable_publish {
                tracing::info!("Running with publish disabled");
                builder.publish = Arc::new(test::NoopPublishFactory);
            }

            if !config.mem_bootstrap {
                tracing::info!("Running with core bootstrap");
                builder.bootstrap = kitsune2_core::factories::CoreBootstrapFactory::create();
            }

            builder
        } else {
            kitsune2::default_builder()
        };

        #[cfg(not(feature = "test_utils"))]
        let mut builder = kitsune2::default_builder();

        builder.auth_material = config.auth_material;

        let evt_sender = Arc::new(std::sync::OnceLock::new());

        builder.peer_meta_store = Arc::new(HolochainPeerMetaStoreFactory {
            getter: config.get_db_peer_meta.clone(),
        });
        builder.op_store = Arc::new(HolochainOpStoreFactory {
            getter: config.get_db_op_store.clone(),
            handler: evt_sender.clone(),
        });
        let preflight = Arc::new(Mutex::new(
            crate::wire::WirePreflightMessage {
                compat: config.compat.clone(),
                agents: Vec::new(),
            }
            .encode()?,
        ));

        // build with whatever bootstrap module is configured,
        // but wrap it in our bootstrap wrapper.
        builder.bootstrap = Arc::new(BootWrapFact {
            compat: config.compat.clone(),
            preflight: preflight.clone(),
            orig: builder.bootstrap,
        });

        // Load default configuration provided by the module factories.
        let builder = builder.with_default_config()?;
        // Then override any configuration values provided by the user.
        if let Some(network_config) = config.network_config {
            builder.config.set_module_config(&network_config)?;
        }

        let pending = Arc::new_cyclic(|this| {
            Mutex::new(Pending {
                this: this.clone(),
                map: HashMap::new(),
            })
        });

        let kitsune = builder.build().await?;

        let kitsune2 = kitsune.clone();
        let db_getter = config.get_db_peer_meta.clone();
        let pruning_task_abort_handle = HolochainP2pActor::spawn_pruning_task(
            config.peer_meta_pruning_interval_ms,
            kitsune2,
            db_getter,
        );

        Ok(Arc::new_cyclic(|this| Self {
            this: this.clone(),
            target_arc_factor: config.target_arc_factor,
            compat: config.compat,
            preflight,
            evt_sender,
            lair_client,
            kitsune,
            pending,
            request_duration_metric: create_p2p_request_duration_metric(),
            pruning_task_abort_handle,
        }))
    }

    // Prunes expired URLs at an interval and checks the peer store for agent infos of unresponsive
    // URLs. If there is an updated agent info since the URL was marked unresponsive, the URL will
    // be pruned.
    fn spawn_pruning_task(
        interval_ms: u64,
        kitsune2: DynKitsune,
        db_getter: GetDbPeerMeta,
    ) -> AbortHandle {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(interval_ms)).await;

                let spaces = kitsune2.list_spaces();
                let pruning_futs = spaces.into_iter().map(|space_id| {
                    let db_getter = db_getter.clone();
                    let kitsune2 = kitsune2.clone();
                    async move {
                        let space = kitsune2.clone().space(space_id.clone()).await?;
                        let peer_store = space.peer_store().clone();
                        let db = db_getter(DnaHash::from_k2_space(&space_id)).await?;
                        // Prune any expired entries.
                        db.write_async(|txn| -> DatabaseResult<()> {
                            let prune_count = txn.execute(sql_peer_meta_store::PRUNE, [])?;
                            tracing::debug!("pruned {prune_count} rows from meta peer store");
                            Ok(())
                        })
                        .await
                        .map_err(HolochainP2pError::other)?;

                        // Get up to date agent infos and compare if there are any up to date ones with
                        // any of the unresponsive URLs.
                        // That would indicate that the URL was unresponsive temporarily and has become
                        // responsive again.
                        let agents = peer_store.get_all().await?;
                        let urls_to_prune = db
                        .read_async(move |txn| -> DatabaseResult<Vec<Url>> {
                            let mut stmt = txn.prepare(sql_peer_meta_store::SELECT_URLS)?;
                            let mut rows = stmt.query(
                            named_params! {":meta_key":format!("{KEY_PREFIX_ROOT}:unresponsive")},
                        )?;
                            let mut urls = Vec::new();
                            while let Some(row) = rows.next()? {
                                // Expecting is safe here, because the inserted values must have been URLs.
                                let peer_url = Url::from_str(row.get::<_, String>(0)?)
                                    .expect("expected valid URL");
                                let timestamp = rmp_serde::from_slice::<kitsune2_api::Timestamp>(
                                    &(row.get::<_, BytesSql>(1)?.0),
                                )?;
                                if let Some(agent) = agents
                                    .iter()
                                    .find(|agent| agent.url == Some(peer_url.clone()))
                                {
                                    if agent.created_at > timestamp {
                                        urls.push(peer_url);
                                    }
                                }
                            }
                            Ok(urls)
                        })
                        .await
                        .map_err(HolochainP2pError::other)?;

                        for url in urls_to_prune {
                            space
                                .peer_meta_store()
                                .delete(url, format!("{KEY_PREFIX_ROOT}:unresponsive"))
                                .await?;
                        }

                        Ok::<_, HolochainP2pError>(())
                    }
                });
                let results = futures::future::join_all(pruning_futs).await;
                for err in results.into_iter().filter_map(Result::err) {
                    tracing::warn!("Pruning peer meta store failed: {err}");
                }
            }
        })
        .abort_handle()
    }

    async fn get_peer_for_loc(
        &self,
        tag: &'static str,
        space: &DynSpace,
        loc: u32,
    ) -> HolochainP2pResult<(AgentPubKey, Url)> {
        let mut agent_list = self.get_peers_for_location(space, loc).await?;

        rand::seq::SliceRandom::shuffle(&mut agent_list[..], &mut rand::thread_rng());
        agent_list
            .into_iter()
            .next()
            .ok_or_else(|| HolochainP2pError::NoPeersForLocation(tag.to_string(), loc))
    }

    async fn get_peers_for_location(
        &self,
        space: &DynSpace,
        loc: u32,
    ) -> HolochainP2pResult<Vec<(AgentPubKey, Url)>> {
        let agent_list = get_remote_agents_near_location(
            space.peer_store().clone(),
            space.local_agent_store().clone(),
            loc,
            1024,
        )
        .await?;

        Ok(agent_list
            .into_iter()
            .filter_map(|a| {
                // much less clear code-wise that way, clippy...
                #[allow(clippy::question_mark)]
                if a.url.is_none() {
                    return None;
                }
                if !a.storage_arc.contains(loc) {
                    return None;
                }
                Some((
                    AgentPubKey::from_k2_agent(&a.agent),
                    a.url.as_ref().unwrap().clone(),
                ))
            })
            .collect::<Vec<_>>())
    }

    /// Check whether a message should be bridged locally to some other agent on this node.
    ///
    /// Checks whether this message is destined for our own URL.
    fn should_bridge(&self, space: &DynSpace, to_url: Url) -> bool {
        space.current_url() == Some(to_url)
    }

    async fn send_notify(
        &self,
        space: &DynSpace,
        to_url: Url,
        req: WireMessage,
    ) -> HolochainP2pResult<()> {
        let req = WireMessage::encode_batch(&[&req])?;
        space.send_notify(to_url, req).await?;
        Ok(())
    }

    async fn send_notify_response(
        &self,
        space_id: SpaceId,
        to_url: Url,
        msg_id: u64,
        res: WireMessage,
    ) -> HolochainP2pResult<()> {
        let space = self
            .kitsune
            .space_if_exists(space_id)
            .await
            .ok_or_else(|| HolochainP2pError::other("no such space"))?;

        if self.should_bridge(&space, to_url.clone()) {
            let r = self.pending.lock().unwrap().map.remove(&msg_id);
            if let Some(r) = r {
                if let Err(err) = r.send(res) {
                    tracing::warn!(?err, "Failed to send bridged response");
                }
            } else {
                tracing::warn!("Attempt to bridge response for unknown msg_id: {msg_id}");
            }
        } else {
            self.send_notify(&space, to_url, res).await?;
        }

        Ok(())
    }

    async fn send_request<O, C>(
        &self,
        tag: &'static str,
        space: &DynSpace,
        to_url: Url,
        msg_id: u64,
        req: WireMessage,
        dna_hash: DnaHash,
        cb: C,
    ) -> HolochainP2pResult<O>
    where
        C: FnOnce(WireMessage) -> HolochainP2pResult<O>,
    {
        let req = WireMessage::encode_batch(&[&req])?;

        let (s, r) = tokio::sync::oneshot::channel();
        self.pending.lock().unwrap().register(msg_id, s);

        let start = std::time::Instant::now();

        if self.should_bridge(space, to_url.clone()) {
            // Note that while bridging is placed here to be supported in the general case, it is only
            // used for the `CallRemote` case. It doesn't make sense to bridge network requests for
            // data, or countersigning messages.
            // For this to work, the request handler must call `send_notify_response`, which will
            // handle relaying the response back to the original sender.

            self.recv_notify(to_url.clone(), dna_hash.to_k2_space(), req)?;
        } else {
            space.send_notify(to_url.clone(), req).await?;
        }

        let record_metric = |error: bool| {
            self.request_duration_metric.record(
                start.elapsed().as_secs_f64(),
                &[
                    opentelemetry_api::KeyValue::new("dna_hash", format!("{:?}", dna_hash)),
                    opentelemetry_api::KeyValue::new("tag", tag),
                    opentelemetry_api::KeyValue::new("url", to_url.as_str().to_string()),
                    opentelemetry_api::KeyValue::new("error", error),
                ],
            );
        };

        match r.await {
            Err(_) => {
                record_metric(true);

                Err(HolochainP2pError::other(format!(
                    "{tag} response channel dropped: likely response timeout"
                )))
            }
            Ok(resp) => {
                let is_err = matches!(resp, WireMessage::ErrorRes { .. });
                record_metric(is_err);

                cb(resp)
            }
        }
    }

    async fn inform_ops_stored(
        &self,
        space_id: SpaceId,
        ops: Vec<StoredOp>,
    ) -> HolochainP2pResult<()> {
        self.kitsune
            .space(space_id)
            .await?
            .inform_ops_stored(ops)
            .await
            .map_err(HolochainP2pError::K2Error)
    }
}

macro_rules! timing_trace_out {
    ($res:ident, $start:ident, $($rest:tt)*) => {
        match &$res {
            Ok(_) => {
                tracing::trace!(
                    target: "NETAUDIT",
                    m = "holochain_p2p",
                    r = "ok",
                    elapsed_s = $start.elapsed().as_secs_f64(),
                    $($rest)*
                );
            }
            Err(err) => {
                tracing::trace!(
                    target: "NETAUDIT",
                    m = "holochain_p2p",
                    ?err,
                    elapsed_s = $start.elapsed().as_secs_f64(),
                    $($rest)*
                );
            }
        }
    };
}

impl actor::HcP2p for HolochainP2pActor {
    #[cfg(feature = "test_utils")]
    fn test_kitsune(&self) -> &DynKitsune {
        &self.kitsune
    }

    fn peer_store(&self, dna_hash: DnaHash) -> BoxFut<'_, HolochainP2pResult<DynPeerStore>> {
        Box::pin(async move {
            Ok(self
                .kitsune
                .space(dna_hash.to_k2_space())
                .await?
                .peer_store()
                .clone())
        })
    }

    fn register_handler(
        &self,
        handler: event::DynHcP2pHandler,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            if let Some(this) = self.this.upgrade() {
                self.evt_sender
                    .set(WrapEvtSender(handler))
                    .map_err(|_| HolochainP2pError::other("handler already set"))?;

                self.kitsune.register_handler(this).await?;

                Ok(())
            } else {
                Err(HolochainP2pError::other(
                    "arc wrapping hc_p2p no longer valid",
                ))
            }
        })
    }

    fn join(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        _maybe_agent_info: Option<AgentInfoSigned>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space = self.kitsune.space(dna_hash.to_k2_space()).await?;

            let local_agent: DynLocalAgent = Arc::new(HolochainP2pLocalAgent::new(
                agent_pub_key,
                DhtArc::FULL,
                self.target_arc_factor,
                self.lair_client.clone(),
            ));

            space.local_agent_join(local_agent).await?;
            Ok(())
        })
    }

    fn leave(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space = self.kitsune.space(dna_hash.to_k2_space()).await?;

            space.local_agent_leave(agent_pub_key.to_k2_agent()).await;
            Ok(())
        })
    }

    fn new_integrated_data(
        &self,
        space_id: SpaceId,
        ops: Vec<StoredOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { self.inform_ops_stored(space_id, ops).await })
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            let byte_count = zome_call_params_serialized.0.len();

            let to_url = space
                .peer_store()
                .get(to_agent.to_k2_agent())
                .await?
                .and_then(|i| i.url.clone())
                .ok_or_else(|| HolochainP2pError::other("call_remote: no url for peer"))?;

            let (msg_id, req) =
                WireMessage::call_remote_req(to_agent, zome_call_params_serialized, signature);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "call_remote",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::CallRemoteRes { response, .. } => Ok(response),
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to call_remote: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, byte_count, a = "send_call_remote");

            out
        })
    }

    fn send_remote_signal(
        &self,
        dna_hash: DnaHash,
        target_payload_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            let byte_count: usize = target_payload_list.iter().map(|(_, p, _)| p.0.len()).sum();

            let mut all = Vec::new();

            for (to_agent, payload, signature) in target_payload_list {
                let to_agent_id = to_agent.to_k2_agent();
                let to_url = match space
                    .peer_store()
                    .get(to_agent_id)
                    .await?
                    .and_then(|i| i.url.clone())
                {
                    Some(to_url) => to_url,
                    None => continue,
                };

                let req = WireMessage::remote_signal_evt(to_agent.clone(), payload, signature);

                if self.should_bridge(&space, to_url.clone()) {
                    if let Err(err) = WireMessage::encode_batch(&[&req])
                        .map(|msg| self.recv_notify(to_url, space_id.clone(), msg))
                    {
                        tracing::debug!(?err, "send_remote_signal failed to bridge call");
                    }
                } else {
                    all.push(async {
                        if let Err(err) = self.send_notify(&space, to_url, req).await {
                            tracing::debug!(?err, "send_remote_signal failed");
                        }
                    });
                }
            }

            let start = std::time::Instant::now();

            if !all.is_empty() {
                // errors handled in individual futures
                let _ = futures::future::join_all(all).await;
            }

            let out = Ok(());

            timing_trace_out!(out, start, byte_count, a = "send_remote_signal");

            out
        })
    }

    fn publish(
        &self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        basis_hash: OpBasis,
        _source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        _timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            use crate::types::event::HcP2pHandler;

            if let Some(reflect_ops) = reflect_ops {
                self.evt_sender
                    .get()
                    .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                    .handle_publish(dna_hash.clone(), request_validation_receipt, reflect_ops)
                    .await?;
            }

            let space = dna_hash.to_k2_space();

            let space = self.kitsune.space(space).await?;

            // -- actually publish the op hashes -- //

            let op_hash_list: Vec<OpId> = op_hash_list.into_iter().map(|h| h.to_k2_op()).collect();

            let urls: std::collections::HashSet<Url> = get_remote_agents_near_location(
                space.peer_store().clone(),
                space.local_agent_store().clone(),
                basis_hash.get_loc(),
                usize::MAX,
            )
            .await?
            .into_iter()
            .filter_map(|info| {
                if info.is_tombstone {
                    return None;
                }
                info.url.clone()
            })
            .collect();

            for url in urls {
                space
                    .publish()
                    .publish_ops(op_hash_list.clone(), url)
                    .await?;
            }

            Ok(())
        })
    }

    fn publish_countersign(
        &self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
        op: ChainOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            let peers = self
                .get_peers_for_location(&space, basis_hash.get_loc())
                .await?;

            let out = futures::future::join_all(peers.into_iter().map(|p| {
                let req = crate::wire::WireMessage::publish_countersign_evt(op.clone());

                Box::pin({
                    let space = space.clone();
                    async move {
                        self.send_notify(&space, p.1, req)
                            .await
                            .map_err(|e| (p.0, e))
                    }
                })
            }))
            .await;

            if out.iter().all(|r| r.is_err()) {
                return Err(HolochainP2pError::other(
                    "publish_countersign failed to publish to any peers",
                ));
            } else {
                // We don't need to publish to everyone, just a neighborhood. Any of those peers
                // can collect signatures and respond. Log any peers that we failed to notify
                // just for debugging purposes.
                out.into_iter()
                    .filter_map(|r| r.err())
                    .for_each(|(agent, err)| {
                        tracing::info!(
                            ?err,
                            ?agent,
                            "publish_countersign failed to publish to a peer"
                        );
                    });
            }

            Ok(())
        })
    }

    fn get(
        &self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireOps>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = dht_hash.get_loc();

            let (to_agent, to_url) = self.get_peer_for_loc("get", &space, loc).await?;

            let (msg_id, req) = crate::wire::WireMessage::get_req(to_agent, dht_hash);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "get",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::GetRes { response, .. } => Ok(vec![response]),
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to get: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_get");

            out
        })
    }

    fn get_meta(
        &self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MetadataSet>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = dht_hash.get_loc();

            let (to_agent, to_url) = self.get_peer_for_loc("get_meta", &space, loc).await?;

            let r_options: event::GetMetaOptions = (&options).into();

            let (msg_id, req) =
                crate::wire::WireMessage::get_meta_req(to_agent, dht_hash, r_options);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "get_meta",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::GetMetaRes { response, .. } => Ok(vec![response]),
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to get_meta: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_get_meta");

            out
        })
    }

    fn get_links(
        &self,
        dna_hash: DnaHash,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireLinkOps>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = link_key.base.get_loc();

            let (to_agent, to_url) = self.get_peer_for_loc("get_links", &space, loc).await?;

            let r_options: event::GetLinksOptions = (&options).into();

            let (msg_id, req) =
                crate::wire::WireMessage::get_links_req(to_agent, link_key, r_options);

            let start = std::time::Instant::now();

            let out =
                self.send_request("get_links", &space, to_url, msg_id, req, dna_hash, |res| {
                    match res {
                        crate::wire::WireMessage::GetLinksRes { response, .. } => {
                            Ok(vec![response])
                        }
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to get_links: {res:?}"
                        ))),
                    }
                })
                .await;

            timing_trace_out!(out, start, a = "send_get_links");

            out
        })
    }

    fn count_links(
        &self,
        dna_hash: DnaHash,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = query.base.get_loc();

            let (to_agent, to_url) = self.get_peer_for_loc("count_links", &space, loc).await?;

            let (msg_id, req) = crate::wire::WireMessage::count_links_req(to_agent, query);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "count_links",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::CountLinksRes { response, .. } => Ok(response),
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to count_links: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_count_links");

            out
        })
    }

    fn get_agent_activity(
        &self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<AgentActivityResponse>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = agent.get_loc();

            let (to_agent, to_url) = self
                .get_peer_for_loc("get_agent_activity", &space, loc)
                .await?;

            let r_options: event::GetActivityOptions = (&options).into();

            let (msg_id, req) =
                crate::wire::WireMessage::get_agent_activity_req(to_agent, agent, query, r_options);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "get_agent_activity",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::GetAgentActivityRes { response, .. } => {
                            Ok(vec![response])
                        }
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to get_agent_activity: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_get_agent_activity");

            out
        })
    }

    fn must_get_agent_activity(
        &self,
        dna_hash: DnaHash,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MustGetAgentActivityResponse>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = author.get_loc();

            let (to_agent, to_url) = self
                .get_peer_for_loc("must_get_agent_activity", &space, loc)
                .await?;

            let (msg_id, req) =
                crate::wire::WireMessage::must_get_agent_activity_req(to_agent, author, filter);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "must_get_agent_activity",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        crate::wire::WireMessage::MustGetAgentActivityRes { response, .. } => {
                            Ok(vec![response])
                        }
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to must_get_agent_activity: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_must_get_agent_activity");

            out
        })
    }

    fn send_validation_receipts(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            let agent_id = to_agent.to_k2_agent();

            let to_url = match space
                .peer_store()
                .get(agent_id)
                .await?
                .and_then(|i| i.url.clone())
            {
                Some(to_url) => to_url,
                None => {
                    return Err(HolochainP2pError::other(
                        "send_validation_receipts could not find url for peer",
                    ))
                }
            };

            // Ideally this would be filtered before here, but to protect against connecting to
            // ourselves, we want a check here.
            if space.current_url() == Some(to_url.clone()) {
                tracing::info!("ignoring send_validation_receipts to ourselves");
                return Ok(());
            }

            let (msg_id, req) =
                WireMessage::send_validation_receipts_req(to_agent.clone(), receipts);

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "send_validation_receipts",
                    &space,
                    to_url,
                    msg_id,
                    req,
                    dna_hash,
                    |res| match res {
                        WireMessage::SendValidationReceiptsRes { .. } => Ok(()),
                        _ => Err(HolochainP2pError::other(format!(
                            "invalid response to send_validation_receipts: {res:?}"
                        ))),
                    },
                )
                .await;

            timing_trace_out!(out, start, a = "send_validation_receipts");

            Ok(())
        })
    }

    fn authority_for_hash(
        &self,
        dna_hash: DnaHash,
        basis: OpBasis,
    ) -> BoxFut<'_, HolochainP2pResult<bool>> {
        Box::pin(async move {
            let loc = basis.get_loc();
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            for agent in space.local_agent_store().get_all().await? {
                if agent.get_cur_storage_arc().contains(loc) {
                    return Ok(true);
                }
            }

            Ok(false)
        })
    }

    fn countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            let mut peer_urls = Vec::with_capacity(agents.len());
            for agent in agents {
                let agent_id = agent.to_k2_agent();
                if let Some(agent_info) = space
                    .peer_store()
                    .get(agent_id.clone())
                    .await
                    .inspect_err(|e| {
                        tracing::error!(
                            ?e,
                            ?agent,
                            "Failed to get peer for countersigning negotiation"
                        );
                    })
                    .ok()
                    .flatten()
                {
                    if let Some(url) = &agent_info.url {
                        peer_urls.push((agent, url.clone()));
                    } else {
                        tracing::error!(?agent, "Peer has no url for countersigning negotiation");
                    }
                }
            }

            let res = futures::future::join_all(peer_urls.into_iter().map(|(agent, url)| {
                let req =
                    WireMessage::countersigning_session_negotiation_evt(agent, message.clone());

                Box::pin({
                    let space = space.clone();
                    let space_id = space_id.clone();
                    async move {
                        if self.should_bridge(&space, url.clone()) {
                            let message = WireMessage::encode_batch(&[&req])?;
                            self.recv_notify(url, space_id, message)?;

                            Ok(())
                        } else {
                            self.send_notify(&space, url, req).await
                        }
                    }
                })
            }))
            .await;

            // We need this to go to all the targets, log any errors and then return an error if any
            // are not okay.
            let any_failed = res.iter().any(|r| r.is_err());
            res.into_iter().filter_map(|r| r.err()).for_each(|err| {
                tracing::error!(
                    ?err,
                    "Failed to send countersigning session negotiation to a peer"
                );
            });

            if any_failed {
                Err(HolochainP2pError::other(
                    "Failed to send countersigning session negotiation to all peers",
                ))
            } else {
                Ok(())
            }
        })
    }

    fn dump_network_metrics(
        &self,
        request: Kitsune2NetworkMetricsRequest,
    ) -> BoxFut<'_, HolochainP2pResult<HashMap<DnaHash, Kitsune2NetworkMetrics>>> {
        Box::pin(async move {
            let spaces = match request.dna_hash {
                Some(dna_hash) => {
                    let space_id = dna_hash.to_k2_space();
                    vec![(
                        space_id.clone(),
                        self.kitsune
                            .space_if_exists(space_id)
                            .await
                            .ok_or_else(|| {
                                K2Error::other(format!("No space found for: {dna_hash:?}"))
                            })?,
                    )]
                }
                None => {
                    let all_space_ids = self.kitsune.list_spaces();
                    let mut spaces = Vec::with_capacity(all_space_ids.len());
                    for space_id in all_space_ids {
                        spaces.push((space_id.clone(), self.kitsune.space(space_id).await?));
                    }

                    spaces
                }
            };

            Ok(
                futures::future::join_all(spaces.into_iter().map(|(space_id, space)| {
                    Box::pin(async move {
                        let fetch_state_summary = space.fetch().get_state_summary().await?;
                        let gossip_state_summary = space
                            .gossip()
                            .get_state_summary(GossipStateSummaryRequest {
                                include_dht_summary: request.include_dht_summary,
                            })
                            .await?;

                        let local_agents = space
                            .local_agent_store()
                            .get_all()
                            .await?
                            .into_iter()
                            .map(|a| LocalAgentSummary {
                                agent: AgentPubKey::from_k2_agent(a.agent()),
                                storage_arc: a.get_cur_storage_arc(),
                                target_arc: a.get_tgt_storage_arc(),
                            })
                            .collect();

                        Ok((
                            DnaHash::from_k2_space(&space_id),
                            Kitsune2NetworkMetrics {
                                fetch_state_summary,
                                gossip_state_summary,
                                local_agents,
                            },
                        ))
                    })
                }))
                .await
                .into_iter()
                .collect::<K2Result<HashMap<_, _>>>()?,
            )
        })
    }

    fn dump_network_stats(&self) -> BoxFut<'_, HolochainP2pResult<TransportStats>> {
        Box::pin(async move { Ok(self.kitsune.transport().await?.dump_network_stats().await?) })
    }

    fn target_arcs(
        &self,
        dna_hash: DnaHash,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<kitsune2_api::DhtArc>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;

            Ok(space
                .local_agent_store()
                .get_all()
                .await?
                .into_iter()
                .map(|a| a.get_tgt_storage_arc())
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn correct_id_loc_calc() {
        // make sure our "Once" kitsune2 setup is executed
        let _ = HolochainP2pActor::create(Default::default(), holochain_keystore::test_keystore())
            .await;

        let h_space = DnaHash::from_raw_32(vec![0xdb; 32]);
        let k_space = h_space.to_k2_space();

        assert_eq!(h_space.get_loc(), k_space.loc());

        let h_agent = AgentPubKey::from_raw_32(vec![0xdc; 32]);
        let k_agent = h_agent.to_k2_agent();

        assert_eq!(h_agent.get_loc(), k_agent.loc());

        let h_op = DhtOpHash::from_raw_32(vec![0xdd; 32]);
        let k_op = h_op.to_k2_op();

        assert_eq!(h_op.get_loc(), k_op.loc());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn correct_id_display() {
        // make sure our "Once" kitsune2 setup is executed
        let _ = HolochainP2pActor::create(Default::default(), holochain_keystore::test_keystore())
            .await;

        let h_space = DnaHash::from_raw_32(vec![0xdb; 32]);
        let k_space = h_space.to_k2_space();

        assert_eq!(h_space.to_string(), k_space.to_string());

        let h_agent = AgentPubKey::from_raw_32(vec![0xdc; 32]);
        let k_agent = h_agent.to_k2_agent();

        assert_eq!(h_agent.to_string(), k_agent.to_string());

        let h_op = DhtOpHash::from_raw_32(vec![0xdd; 32]);
        let k_op = h_op.to_k2_op();

        assert_eq!(h_op.to_string(), k_op.to_string());
    }
}
