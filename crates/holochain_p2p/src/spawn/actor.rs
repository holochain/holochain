#![allow(clippy::too_many_arguments)]

/// Hard-code for now.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

use crate::*;
use kitsune2_api::*;
use std::collections::HashMap;
use std::sync::{Mutex, Weak};

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
                    dna_hash, // from,
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
        countersigning_session: bool,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        let op_count = ops.len();
        timing_trace!(
            true,
            {
                self.0.handle_publish(dna_hash, request_validation_receipt, countersigning_session, ops)
            }, %op_count, a = "recv_publish")
    }

    fn handle_get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        timing_trace!(
            true,
            { self.0.handle_get(dna_hash, to_agent, dht_hash, options) },
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

    fn handle_validation_receipts(
        &self,
        dna_hash: DnaHash,
        dht_op_list: Vec<DhtOpHash>,
    ) -> BoxFut<'_, HolochainP2pResult<ValidationReceiptBundle>> {
        timing_trace!(
            false,
            { self.0.handle_validation_receipts(dna_hash, dht_op_list) },
            a = "recv_validation_receipt",
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
    preflight: Arc<std::sync::Mutex<bytes::Bytes>>,
    evt_sender: Arc<std::sync::OnceLock<WrapEvtSender>>,
    lair_client: holochain_keystore::MetaLairClient,
    kitsune: kitsune2_api::DynKitsune,
    pending: Arc<Mutex<Pending>>,
}

impl std::fmt::Debug for HolochainP2pActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainP2pActor").finish()
    }
}

const EVT_REG_ERR: &str = "event handler not registered";

impl kitsune2_api::SpaceHandler for HolochainP2pActor {
    fn recv_notify(&self, from_peer: Url, space: SpaceId, data: bytes::Bytes) -> K2Result<()> {
        for msg in crate::wire::WireMessage::decode_batch(&data).map_err(|err| {
            K2Error::other_src("decode incoming holochain_p2p wire message batch", err)
        })? {
            // NOTE: spawning a task here could lead to memory issues
            //       in the case of DoS messaging, consider some kind
            //       of queue or semaphore.
            let from_peer = from_peer.clone();
            let space = space.clone();
            let evt_sender = self.evt_sender.clone();
            let kitsune = self.kitsune.clone();
            let pending = self.pending.clone();
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
                    | ValidationReceiptRes { msg_id, .. } => {
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;
                        if let Err(err) = kitsune
                            .space_if_exists(space)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending call remote response");
                        }
                    }
                    GetReq {
                        msg_id,
                        to_agent,
                        dht_hash,
                        options,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_get(dna_hash, to_agent, dht_hash, options)
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
                            .space_if_exists(space)
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                            .space_if_exists(space)
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                            .space_if_exists(space)
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                            .space_if_exists(space)
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                            .space_if_exists(space)
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
                        let dna_hash = DnaHash::from_k2_space(&space);
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
                            .space_if_exists(space)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending must_get_agent_activity response");
                        }
                    }
                    ValidationReceiptReq {
                        msg_id,
                        dht_op_list,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space);
                        let resp = match evt_sender
                            .get()
                            .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                            .handle_validation_receipts(dna_hash, dht_op_list)
                            .await
                        {
                            Ok(receipts) => ValidationReceiptRes { msg_id, receipts },
                            Err(err) => ErrorRes {
                                msg_id,
                                error: format!("{err:?}"),
                            },
                        };
                        let resp = crate::wire::WireMessage::encode_batch(&[&resp])?;

                        if let Err(err) = kitsune
                            .space_if_exists(space)
                            .await
                            .ok_or_else(|| HolochainP2pError::other("no such space"))?
                            .send_notify(from_peer, resp)
                            .await
                        {
                            tracing::debug!(?err, "Error sending validation_receipt response");
                        }
                    }
                    RemoteSignalEvt {
                        to_agent,
                        zome_call_params_serialized,
                        signature,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space);
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

            builder
        } else {
            kitsune2::default_builder()
        };

        #[cfg(not(feature = "test_utils"))]
        let mut builder = kitsune2::default_builder();

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

        Ok(Arc::new_cyclic(|this| Self {
            this: this.clone(),
            target_arc_factor: config.target_arc_factor,
            compat: config.compat,
            preflight,
            evt_sender,
            lair_client,
            kitsune,
            pending,
        }))
    }

    async fn get_peer_for_loc(
        &self,
        tag: &'static str,
        space: &DynSpace,
        loc: u32,
    ) -> HolochainP2pResult<(AgentPubKey, Url)> {
        let agent_list = space.peer_store().get_near_location(loc, 1024).await?;

        let mut agent_list = agent_list
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
            .collect::<Vec<_>>();

        rand::seq::SliceRandom::shuffle(&mut agent_list[..], &mut rand::thread_rng());
        agent_list
            .into_iter()
            .next()
            .ok_or_else(|| HolochainP2pError::NoPeersForLocation(tag.to_string(), loc))
    }

    async fn send_notify(
        &self,
        space: &DynSpace,
        to_url: Url,
        req: crate::wire::WireMessage,
    ) -> HolochainP2pResult<()> {
        let req = crate::wire::WireMessage::encode_batch(&[&req])?;
        space.send_notify(to_url, req).await?;
        Ok(())
    }

    async fn send_request<O, C>(
        &self,
        tag: &'static str,
        space: &DynSpace,
        to_url: Url,
        msg_id: u64,
        req: crate::wire::WireMessage,
        cb: C,
    ) -> HolochainP2pResult<O>
    where
        C: FnOnce(crate::wire::WireMessage) -> HolochainP2pResult<O>,
    {
        let req = crate::wire::WireMessage::encode_batch(&[&req])?;

        let (s, r) = tokio::sync::oneshot::channel();
        self.pending.lock().unwrap().register(msg_id, s);

        space.send_notify(to_url, req).await?;

        match r.await {
            Err(_) => Err(HolochainP2pError::other(format!(
                "{tag} response channel dropped: likely response timeout"
            ))),
            Ok(resp) => cb(resp),
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

    /* -----------------
     * saving so we can implement similiar stuff later

    /// receiving an incoming publish from a remote node
    fn handle_incoming_publish(
        &mut self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        ops: Vec<holochain_types::dht_op::DhtOp>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .publish(
                    dna_hash,
                    request_validation_receipt,
                    countersigning_session,
                    ops,
                )
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_incoming_countersigning_session_negotiation(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: CountersigningSessionNegotiationMessage,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .countersigning_session_negotiation(dna_hash, to_agent, message)
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    ------------------ */
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

            let (msg_id, req) = crate::wire::WireMessage::call_remote_req(
                to_agent,
                zome_call_params_serialized,
                signature,
            );

            let start = std::time::Instant::now();

            let out = self
                .send_request(
                    "call_remote",
                    &space,
                    to_url,
                    msg_id,
                    req,
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
                let to_url = match space
                    .peer_store()
                    .get(to_agent.to_k2_agent())
                    .await?
                    .and_then(|i| i.url.clone())
                {
                    Some(to_url) => to_url,
                    None => continue,
                };

                let req = crate::wire::WireMessage::remote_signal_evt(
                    to_agent.clone(),
                    payload,
                    signature,
                );

                all.push(async {
                    if let Err(err) = self.send_notify(&space, to_url, req).await {
                        tracing::debug!(?err, "send_remote_signal failed");
                    }
                });
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
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        _source: AgentPubKey,
        op_hash_list: Vec<DhtOpHash>,
        _timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            use crate::types::event::HcP2pHandler;

            // This single function is/was a weird mix of a bunch of strange
            // operations. We're trying to narrow it down now.
            //
            // - `reflect_ops` back to holochain
            //   - TODO - remove this (fix anything it breaks)
            // - actually publish `op_hash_list`
            //   - This is the core functionality we want to keep here
            // - send a notification to peers that we'd like validation receipts
            //   - moving this to a separate call
            // - i have no idea at all what the countersigning bool is for
            //   - moving this to a separate call

            // -- handle the bizzarre reflection thing -- //

            if let Some(reflect_ops) = reflect_ops {
                self.evt_sender
                    .get()
                    .ok_or_else(|| HolochainP2pError::other(EVT_REG_ERR))?
                    .handle_publish(
                        dna_hash.clone(),
                        request_validation_receipt,
                        countersigning_session,
                        reflect_ops,
                    )
                    .await?;
            }

            let space = dna_hash.to_k2_space();

            let space = self.kitsune.space(space).await?;

            // -- actually publish the op hashes -- //

            let op_hash_list: Vec<OpId> = op_hash_list.into_iter().map(|h| h.to_k2_op()).collect();

            let urls: std::collections::HashSet<Url> = space
                .peer_store()
                .get_near_location(basis_hash.get_loc(), usize::MAX)
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
        _dna_hash: DnaHash,
        _flag: bool,
        _basis_hash: holo_hash::OpBasis,
        _op: DhtOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            tracing::error!("publish_countersign is currently a STUB in holochain_p2p--the countersigning feature is unstable");
            Ok(())
        })
    }

    fn get(
        &self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireOps>>> {
        Box::pin(async move {
            let space_id = dna_hash.to_k2_space();
            let space = self.kitsune.space(space_id.clone()).await?;
            let loc = dht_hash.get_loc();

            let (to_agent, to_url) = self.get_peer_for_loc("get", &space, loc).await?;

            let r_options: event::GetOptions = (&options).into();

            let (msg_id, req) = crate::wire::WireMessage::get_req(to_agent, dht_hash, r_options);

            let start = std::time::Instant::now();

            let out = self
                .send_request("get", &space, to_url, msg_id, req, |res| match res {
                    crate::wire::WireMessage::GetRes { response, .. } => Ok(vec![response]),
                    _ => Err(HolochainP2pError::other(format!(
                        "invalid response to get: {res:?}"
                    ))),
                })
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
                .send_request("get_meta", &space, to_url, msg_id, req, |res| match res {
                    crate::wire::WireMessage::GetMetaRes { response, .. } => Ok(vec![response]),
                    _ => Err(HolochainP2pError::other(format!(
                        "invalid response to get_meta: {res:?}"
                    ))),
                })
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

            let out = self
                .send_request("get_links", &space, to_url, msg_id, req, |res| match res {
                    crate::wire::WireMessage::GetLinksRes { response, .. } => Ok(vec![response]),
                    _ => Err(HolochainP2pError::other(format!(
                        "invalid response to get_links: {res:?}"
                    ))),
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

    fn get_validation_receipts(
        &self,
        dna_hash: DnaHash,
        basis_hash: holo_hash::OpBasis,
        op_hash_list: Vec<DhtOpHash>,
        exclude_list: Vec<AgentPubKey>,
        limit: usize,
    ) -> BoxFut<'_, HolochainP2pResult<ValidationReceiptBundle>> {
        Box::pin(async move {
            let exclude_list: Vec<AgentId> =
                exclude_list.into_iter().map(|a| a.to_k2_agent()).collect();
            let space = dna_hash.to_k2_space();
            let space = self.kitsune.space(space).await?;
            let urls: std::collections::HashSet<Url> = space
                .peer_store()
                .get_near_location(basis_hash.get_loc(), usize::MAX)
                .await?
                .into_iter()
                .filter_map(|info| {
                    if info.is_tombstone {
                        return None;
                    }
                    if exclude_list.contains(&info.agent) {
                        return None;
                    }
                    info.url.clone()
                })
                .collect();

            if urls.is_empty() {
                return Ok(<Vec<SignedValidationReceipt>>::new().into());
            }

            let mut recv = Vec::new();

            for url in urls.into_iter().take(limit) {
                let (msg_id, req) =
                    crate::wire::WireMessage::validation_receipt_req(op_hash_list.clone());
                let req = crate::wire::WireMessage::encode_batch(&[&req])?;
                let (s, r) = tokio::sync::oneshot::channel();
                recv.push(r);
                self.pending.lock().unwrap().register(msg_id, s);
                space.send_notify(url, req).await?;
            }

            let mut out: Vec<SignedValidationReceipt> = Vec::new();

            for bundle in futures::future::join_all(recv).await {
                let mut bundle = match bundle {
                    Ok(wire::WireMessage::ValidationReceiptRes { receipts, .. }) => {
                        receipts.into_iter().collect::<Vec<_>>()
                    }
                    Ok(resp) => {
                        tracing::debug!(?resp, "bad response fetching validation receipts");
                        continue;
                    }
                    Err(err) => {
                        tracing::debug!(?err, "error responce fetching validation receipts");
                        continue;
                    }
                };
                out.append(&mut bundle);
            }

            Ok(out.into())
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
        _dna_hash: DnaHash,
        _agents: Vec<AgentPubKey>,
        _message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            tracing::error!("countersigning_session_negotiation is currently a STUB in holochain_p2p--the countersigning feature is unstable");
            Ok(())
        })
    }

    fn dump_network_metrics(
        &self,
        _dna_hash: Option<DnaHash>,
    ) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async move {
            tracing::error!("dump_network_metrics is currently a STUB in holochain_p2p--deferring until at least CI is building again");
            // not sure if this was json, but make it an empty obj just in case : )
            Ok("{}".into())
        })
    }

    fn dump_network_stats(&self) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async move {
            tracing::error!("dump_network_stats is currently a STUB in holochain_p2p--deferring until at least CI is building again");
            // not sure if this was json, but make it an empty obj just in case : )
            Ok("{}".into())
        })
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

    /* ---------------------------
     * keeping the original code here, because we will need some
     * of this logic when doing the actual implementations


    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_publish(
        &mut self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<OpHashSized>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> HolochainP2pHandlerResult<()> {
        let op_hash_count = op_hash_list.len();

        use kitsune_p2p_types::KitsuneTimeout;

        let source = source.into_kitsune();
        let space = dna_hash.clone().into_kitsune();
        let basis = basis_hash.to_kitsune();
        let timeout = match timeout_ms {
            Some(ms) => KitsuneTimeout::from_millis(ms),
            None => self.config.tuning_params.implicit_timeout(),
        };

        let fetch_context = FetchContext::default()
            .with_request_validation_receipt(request_validation_receipt)
            .with_countersigning_session(countersigning_session);

        let kitsune_p2p = self.kitsune_p2p.clone();
        let host = self.host.clone();
        let evt_sender = self.evt_sender.clone();
        timing_trace_out!(
            async move {
                if let Some(reflect_ops) = reflect_ops {
                    let _ = evt_sender
                        .publish(
                            dna_hash,
                            request_validation_receipt,
                            countersigning_session,
                            reflect_ops,
                        )
                        .await;
                }

                // little awkward, but we need the side-effects of reporting
                // the context back to the host api here:
                if let Err(err) = host
                    .check_op_data(
                        space.clone(),
                        op_hash_list.iter().map(|x| x.data()).collect(),
                        Some(fetch_context),
                    )
                    .await
                {
                    tracing::warn!(?err);
                }

                kitsune_p2p
                    .broadcast(
                        space.clone(),
                        basis.clone(),
                        timeout,
                        BroadcastData::Publish {
                            source,
                            transfer_method: kitsune_p2p_fetch::TransferMethod::Publish,
                            op_hash_list,
                            context: fetch_context,
                        },
                    )
                    .await?;
                Ok(())
            },
            op_hash_count,
            a = "send_publish"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_publish_countersign(
        &mut self,
        dna_hash: DnaHash,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let basis = basis_hash.to_kitsune();
        let timeout = self.config.tuning_params.implicit_timeout();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let payload = crate::wire::WireMessage::publish_countersign(flag, op).encode()?;

            kitsune_p2p
                .broadcast(space, basis, timeout, BroadcastData::User(payload))
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_countersigning_session_negotiation(
        &mut self,
        dna_hash: DnaHash,
        agents: Vec<AgentPubKey>,
        message: CountersigningSessionNegotiationMessage,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agents = agents.into_iter().map(|a| a.into_kitsune()).collect();

        let timeout = self.config.tuning_params.implicit_timeout();

        let payload =
            crate::wire::WireMessage::countersigning_session_negotiation(message).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .targeted_broadcast(space, agents, timeout, payload, false)
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    fn handle_dump_network_metrics(
        &mut self,
        dna_hash: Option<DnaHash>,
    ) -> HolochainP2pHandlerResult<String> {
        let space = dna_hash.map(|h| h.into_kitsune());
        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            serde_json::to_string_pretty(&kitsune_p2p.dump_network_metrics(space).await?)
                .map_err(HolochainP2pError::other)
        }
        .boxed()
        .into())
    }

    fn handle_dump_network_stats(&mut self) -> HolochainP2pHandlerResult<String> {
        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            serde_json::to_string_pretty(&kitsune_p2p.dump_network_stats().await?)
                .map_err(HolochainP2pError::other)
        }
        .boxed()
        .into())
    }

    fn handle_get_diagnostics(
        &mut self,
        dna_hash: DnaHash,
    ) -> HolochainP2pHandlerResult<KitsuneDiagnostics> {
        let space = dna_hash.into_kitsune();
        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .get_diagnostics(space)
                .await
                .map_err(HolochainP2pError::other)
        }
        .boxed()
        .into())
    }

    ----------------- */
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
