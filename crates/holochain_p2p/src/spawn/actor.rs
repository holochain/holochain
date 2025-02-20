#![allow(clippy::too_many_arguments)]

/// Hard-code for now.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

use crate::*;
use kitsune2_api::*;
use std::sync::{Mutex, Weak};
use std::collections::HashMap;

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

#[derive(Clone, Debug)]
struct WrapEvtSender(event::DynHcP2pHandler);

impl event::HcP2pHandler for WrapEvtSender {
    fn call_remote(
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
                self.0.call_remote(
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

    fn publish(
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
                self.0.publish(dna_hash, request_validation_receipt, countersigning_session, ops)
            }, %op_count, a = "recv_publish")
    }

    fn get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireOps>> {
        timing_trace!(
            true,
            { self.0.get(dna_hash, to_agent, dht_hash, options) },
            a = "recv_get",
        )
    }

    fn get_meta(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<MetadataSet>> {
        timing_trace!(
            true,
            { self.0.get_meta(dna_hash, to_agent, dht_hash, options) },
            a = "recv_get_meta",
        )
    }

    fn get_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<WireLinkOps>> {
        timing_trace!(
            true,
            { self.0.get_links(dna_hash, to_agent, link_key, options) },
            a = "recv_get_links",
        )
    }

    fn count_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        timing_trace!(
            true,
            { self.0.count_links(dna_hash, to_agent, query) },
            a = "recv_count_links"
        )
    }

    fn get_agent_activity(
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
                    .get_agent_activity(dna_hash, to_agent, agent, query, options)
            },
            a = "recv_get_agent_activity",
        )
    }

    fn must_get_agent_activity(
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
                    .must_get_agent_activity(dna_hash, to_agent, agent, filter)
            },
            a = "recv_must_get_agent_activity",
        )
    }

    fn validation_receipts_received(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        timing_trace!(
            false,
            {
                self.0
                    .validation_receipts_received(dna_hash, to_agent, receipts)
            },
            a = "recv_validation_receipt_received",
        )
    }

    fn countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        timing_trace!(
            false,
            {
                self.0
                    .countersigning_session_negotiation(dna_hash, to_agent, message)
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
                tokio::time::sleep(TIMEOUT).await;
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
    evt_sender: WrapEvtSender,
    lair_client: holochain_keystore::MetaLairClient,
    kitsune: Mutex<Option<kitsune2_api::DynKitsune>>,
    pending: Arc<Mutex<Pending>>,
}

impl std::fmt::Debug for HolochainP2pActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolochainP2pActor").finish()
    }
}

impl kitsune2_api::SpaceHandler for HolochainP2pActor {
    fn recv_notify(
        &self,
        _to_agent: AgentId,
        _from_agent: AgentId,
        space: SpaceId,
        data: bytes::Bytes,
    ) -> K2Result<()> {
        for msg in crate::wire::WireMessage::decode_batch(&data).map_err(|err| {
            K2Error::other_src("decode incoming holochain_p2p wire message batch", err)
        })? {
            // NOTE: spawning a task here could lead to memory issues
            //       in the case of DoS messaging, consider some kind
            //       of queue or semaphore.
            let evt_sender = self.evt_sender.clone();
            let kitsune = self.kitsune()?;
            let pending = self.pending.clone();
            let space = space.clone();
            tokio::task::spawn(async move {
                use crate::event::HcP2pHandler;
                use crate::wire::WireMessage::*;
                match msg {
                    CallRemoteReq {
                        msg_id,
                        to_agent,
                        zome_call_params_serialized,
                        signature,
                    } => {
                        let dna_hash = DnaHash::from_k2_space(&space);
                        let _response = evt_sender.call_remote(
                            dna_hash,
                            to_agent,
                            zome_call_params_serialized,
                            signature,
                        ).await;
                        todo!()
                    }
                    CallRemoteRes { msg_id, .. } => {
                        if let Some(resp) = pending.lock().unwrap().respond(msg_id) {
                            let _ = resp.send(msg);
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
                        let _response = evt_sender.call_remote(
                            dna_hash,
                            to_agent,
                            zome_call_params_serialized,
                            signature,
                        ).await;
                    }
                }
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
                Err(kitsune2_api::K2Error::other("instance has been dropped"))
            }
        })
    }
}

impl HolochainP2pActor {
    /// constructor
    pub async fn create(
        db_peer_meta: DbWrite<DbKindPeerMetaStore>,
        db_op: DbWrite<DbKindDht>,
        handler: event::DynHcP2pHandler,
        lair_client: holochain_keystore::MetaLairClient,
        _compat: NetworkCompatParams,
    ) -> HolochainP2pResult<actor::DynHcP2p> {
        let builder = Builder {
            peer_meta_store: Arc::new(HolochainPeerMetaStoreFactory { db: db_peer_meta }),
            op_store: Arc::new(HolochainOpStoreFactory {
                db: db_op,
                handler: handler.clone(),
            }),
            ..kitsune2::default_builder()
        }
        .with_default_config()?;

        let pending = Arc::new_cyclic(|this| Mutex::new(Pending {
            this: this.clone(),
            map: HashMap::new(),
        }));

        let this = Arc::new_cyclic(|this| Self {
            this: this.clone(),
            evt_sender: WrapEvtSender(handler),
            lair_client,
            kitsune: Mutex::new(None),
            pending,
        });

        let kitsune = builder.build(this.clone()).await?;

        *this.kitsune.lock().unwrap() = Some(kitsune);

        Ok(this)
        /*
        let mut bytes = vec![];
        kitsune_p2p_types::codec::rmp_encode(&mut bytes, &compat)
            .map_err(HolochainP2pError::other)?;

        let preflight_user_data = PreflightUserData {
            bytes: bytes.clone(),
            comparator: Box::new(move |url, mut recvd_bytes| {
                if bytes.as_slice() != recvd_bytes {
                    let common = "Cannot complete preflight handshake with peer because network compatibility params don't match";
                    Err(
                        match kitsune_p2p_types::codec::rmp_decode::<_, NetworkCompatParams>(
                            &mut recvd_bytes,
                        ) {
                            Ok(theirs) => {
                                format!("{common}. ours={compat:?}, theirs={theirs:?}, url={url}")
                            }
                            Err(err) => {
                                format!(
                                "{common}. (Can't decode peer's sent hash.) url={url}, err={err}"
                            )
                            }
                        },
                    )
                } else {
                    Ok(())
                }
            }),
        };
        */
    }

    fn kitsune(&self) -> K2Result<DynKitsune> {
        match &*self.kitsune.lock().unwrap() {
            Some(kitsune) => Ok(kitsune.clone()),
            None => Err(K2Error::other("uninitialized")),
        }
    }

    /* -----------------
     * saving so we can implement similiar stuff later

    /// receiving an incoming request from a remote node
    #[allow(clippy::too_many_arguments)]
    fn handle_incoming_call_remote(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .call_remote(dna_hash, to_agent, zome_call_params_serialized, signature)
                .await;
            res.map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming get request from a remote node
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self, dna_hash, to_agent, dht_hash, options), level = "trace")
    )]
    fn handle_incoming_get(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender.get(dna_hash, to_agent, dht_hash, options).await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .instrument(tracing::debug_span!("incoming_get_task"))
        .boxed()
        .into())
    }

    /// receiving an incoming get_meta request from a remote node
    fn handle_incoming_get_meta(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .get_meta(dna_hash, to_agent, dht_hash, options)
                .await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming get_links request from a remote node
    fn handle_incoming_get_links(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .get_links(dna_hash, to_agent, link_key, options)
                .await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    fn handle_incoming_count_links(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: WireLinkQuery,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender.count_links(dna_hash, to_agent, query).await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming get_links request from a remote node
    fn handle_incoming_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .get_agent_activity(dna_hash, to_agent, agent, query, options)
                .await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming must_get_agent_activity request from a remote node
    fn handle_incoming_must_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .must_get_agent_activity(dna_hash, to_agent, agent, filter)
                .await;
            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

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

    /// receiving an incoming validation receipt from a remote node
    fn handle_incoming_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .validation_receipts_received(dna_hash, agent_pub_key, receipts)
                .await?;

            // validation receipts don't need a response
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

/* -----------------------
 * Some of the functionality in this comment block will need to
 * be implemented when we receive a space notify from kitsune2

impl kitsune_p2p::event::KitsuneP2pEventHandler for HolochainP2pActor {
    /// Handle an incoming call.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self, space, to_agent, payload), level = "trace")
    )]
    fn handle_call(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<u8>> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);

        let request =
            crate::wire::WireMessage::decode(payload.as_ref()).map_err(HolochainP2pError::from)?;

        match request {
            crate::wire::WireMessage::CallRemote {
                to_agent,
                zome_call_params_serialized,
                signature,
            } => self.handle_incoming_call_remote(
                space,to_agent,  zome_call_params_serialized,signature,
            ),
            crate::wire::WireMessage::CallRemoteMulti {
                to_agents,
            } => {
                match to_agents
                    .into_iter()
                    .find(|( agent,_zome_call_payload, _signature)| agent == &to_agent)
                {
                    Some((to_agent, zome_call_payload, signature)) => self.handle_incoming_call_remote(
                        space, to_agent,zome_call_payload, signature
                    ),
                    None => Err(HolochainP2pError::RoutingAgentError(to_agent).into()),
                }
            }
            crate::wire::WireMessage::Get { dht_hash, options } => {
                self.handle_incoming_get(space, to_agent, dht_hash, options)
            }
            crate::wire::WireMessage::GetMeta { dht_hash, options } => {
                self.handle_incoming_get_meta(space, to_agent, dht_hash, options)
            }
            crate::wire::WireMessage::GetLinks { link_key, options } => {
                self.handle_incoming_get_links(space, to_agent, link_key, options)
            }
            WireMessage::CountLinks { query } => {
                self.handle_incoming_count_links(space, to_agent, query)
            }
            crate::wire::WireMessage::GetAgentActivity {
                agent,
                query,
                options,
            } => self.handle_incoming_get_agent_activity(space, to_agent, agent, query, options),
            crate::wire::WireMessage::MustGetAgentActivity { agent, filter } => {
                self.handle_incoming_must_get_agent_activity(space, to_agent, agent, filter)
            }
            crate::wire::WireMessage::ValidationReceipts { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid: validation receipts are now notifications rather than requests, please upgrade".to_string(),
                )
                    .into())
            }
            // holochain_p2p only broadcasts this message.
            crate::wire::WireMessage::CountersigningSessionNegotiation { .. }
            | crate::wire::WireMessage::PublishCountersign { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid: countersigning messages are broadcast, not requests".to_string(),
                )
                .into())
            }
        }
    }

    /// Handle an incoming notify.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_notify(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);

        let request =
            crate::wire::WireMessage::decode(payload.as_ref()).map_err(HolochainP2pError::from)?;

        match request {
            // error on these call type messages
            crate::wire::WireMessage::Get { .. }
            | crate::wire::WireMessage::GetMeta { .. }
            | crate::wire::WireMessage::GetLinks { .. }
            | crate::wire::WireMessage::CountLinks { .. }
            | crate::wire::WireMessage::GetAgentActivity { .. }
            | crate::wire::WireMessage::MustGetAgentActivity { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid call type message in a notify".to_string(),
                )
                .into())
            }
            crate::wire::WireMessage::CallRemote {
                to_agent,
                zome_call_params_serialized,
                signature,
            } => {
                let fut = self.handle_incoming_call_remote(
                    space,
                    to_agent,
                    zome_call_params_serialized,
                    signature,
                );
                Ok(async move {
                    let _ = fut?.await?;
                    Ok(())
                }
                .boxed()
                .into())
            }
            crate::wire::WireMessage::CallRemoteMulti { to_agents } => {
                match to_agents
                    .into_iter()
                    .find(|(agent, _zome_call_payload, _signature)| agent == &to_agent)
                {
                    Some((to_agent, zome_call_payload, signature)) => {
                        let fut = self.handle_incoming_call_remote(
                            space,
                            to_agent,
                            zome_call_payload,
                            signature,
                        );
                        Ok(async move {
                            let _ = fut?.await?;
                            Ok(())
                        }
                        .boxed()
                        .into())
                    }
                    None => Err(HolochainP2pError::RoutingAgentError(to_agent).into()),
                }
            }
            WireMessage::ValidationReceipts { receipts } => {
                self.handle_incoming_validation_receipt(space, to_agent, receipts)
            }
            crate::wire::WireMessage::CountersigningSessionNegotiation { message } => {
                self.handle_incoming_countersigning_session_negotiation(space, to_agent, message)
            }
            crate::wire::WireMessage::PublishCountersign { flag, op } => {
                self.handle_incoming_publish(space, false, flag, vec![op])
            }
        }
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_receive_ops(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        ops: Vec<KOp>,
        context: Option<FetchContext>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);

        let ops = ops
            .into_iter()
            .map(|op_data| {
                let op = crate::wire::WireDhtOpData::decode(op_data.0.clone())
                    .map_err(HolochainP2pError::from)?
                    .op_data;

                Ok(op)
            })
            .collect::<Result<_, HolochainP2pError>>()?;
        if let Some(context) = context {
            self.handle_incoming_publish(
                space,
                context.has_request_validation_receipt(),
                context.has_countersigning_session(),
                ops,
            )
        } else {
            self.handle_incoming_publish(space, false, false, ops)
        }
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_query_op_hashes(
        &mut self,
        input: kitsune_p2p::event::QueryOpHashesEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<
        Option<(Vec<Arc<kitsune_p2p::KitsuneOpHash>>, TimeWindowInclusive)>,
    > {
        let kitsune_p2p::event::QueryOpHashesEvt {
            space,
            arc_set,
            window,
            max_ops,
            include_limbo,
        } = input;
        let space = DnaHash::from_kitsune(&space);

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .query_op_hashes(space, arc_set, window, max_ops, include_limbo)
                .await?
                .map(|(h, time)| (h.into_iter().map(|h| h.into_kitsune()).collect(), time)))
        }
        .boxed()
        .into())
    }

    #[allow(clippy::needless_collect)]
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_fetch_op_data(
        &mut self,
        input: kitsune_p2p::event::FetchOpDataEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<(Arc<kitsune_p2p::KitsuneOpHash>, KOp)>>
    {
        let kitsune_p2p::event::FetchOpDataEvt { space, query } = input;
        let space = DnaHash::from_kitsune(&space);
        let query = FetchOpDataQuery::from_kitsune(query);

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let mut out = vec![];
            for (op_hash, dht_op) in evt_sender.fetch_op_data(space.clone(), query).await? {
                out.push((
                    op_hash.into_kitsune(),
                    KitsuneOpData::new(
                        crate::wire::WireDhtOpData { op_data: dht_op }
                            .encode()
                            .map_err(kitsune_p2p::KitsuneP2pError::other)?,
                    ),
                ));
            }
            Ok(out)
        }
        .boxed()
        .into())
    }
}
----------------------- */

macro_rules! timing_trace_out {
    ($code:expr, $($rest:tt)*) => {{
        let __start = std::time::Instant::now();
        let __out = $code.await;
        let __elapsed_s = __start.elapsed().as_secs_f64();
        match &__out {
            Ok(_) => {
                tracing::trace!(
                    target: "NETAUDIT",
                    m = "holochain_p2p",
                    r = "ok",
                    elapsed_s = __elapsed_s,
                    $($rest)*
                );
            }
            Err(err) => {
                tracing::trace!(
                    target: "NETAUDIT",
                    m = "holochain_p2p",
                    ?err,
                    elapsed_s = __elapsed_s,
                    $($rest)*
                );
            }
        }
        __out
    }};
}

impl actor::HcP2p for HolochainP2pActor {
    fn join(
        &self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        _maybe_agent_info: Option<AgentInfoSigned>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let kitsune = self.kitsune()?;
            let space = kitsune.space(dna_hash.to_k2_space()).await?;

            let local_agent: DynLocalAgent = Arc::new(HolochainP2pLocalAgent::new(
                agent_pub_key,
                DhtArc::FULL,
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
            let kitsune = self.kitsune()?;
            let space = kitsune.space(dna_hash.to_k2_space()).await?;

            space.local_agent_leave(agent_pub_key.to_k2_agent()).await;
            Ok(())
        })
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> BoxFut<'_, HolochainP2pResult<SerializedBytes>> {
        Box::pin(async move {
            let kitsune = self.kitsune()?;
            let space_id = dna_hash.to_k2_space();
            let space = kitsune.space(space_id.clone()).await?;

            let byte_count = zome_call_params_serialized.0.len();

            let k2_agent = to_agent.to_k2_agent();
            let no_from_agent = AgentId::from(bytes::Bytes::new());

            let (msg_id, req) = crate::wire::WireMessage::call_remote_req(
                to_agent,
                zome_call_params_serialized,
                signature,
            );
            // someday we may support batching
            let req = crate::wire::WireMessage::encode_batch(&[&req])?;

            timing_trace_out!(
                async move {
                    let (s, r) = tokio::sync::oneshot::channel();
                    self.pending.lock().unwrap().register(msg_id, s);

                    space.send_notify(k2_agent, no_from_agent, req).await?;

                    match r.await {
                        Err(_) => {
                            Err(K2Error::other("call_remote response channel dropped: likely response timeout").into())
                        }
                        Ok(resp) => {
                            if let crate::wire::WireMessage::CallRemoteRes { response, .. } = resp {
                                Ok(response)
                            } else {
                                Err(K2Error::other("invalid response message type").into())
                            }
                        }
                    }
                },
                byte_count,
                a = "send_call_remote"
            )
        })
    }

    fn send_remote_signal(
        &self,
        dna_hash: DnaHash,
        target_payload_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move {
            let kitsune = self.kitsune()?;
            let space_id = dna_hash.to_k2_space();
            let space = kitsune.space(space_id.clone()).await?;

            let byte_count: usize = target_payload_list.iter().map(|(_, p, _)| p.0.len()).sum();

            let mut all = Vec::new();

            for (to_agent, payload, signature) in target_payload_list {
                let k2_agent = to_agent.to_k2_agent();
                let no_from_agent = AgentId::from(bytes::Bytes::new());

                let req = crate::wire::WireMessage::remote_signal_evt(
                    to_agent.clone(),
                    payload,
                    signature,
                );
                // someday we may support batching
                let req = crate::wire::WireMessage::encode_batch(&[&req])?;

                all.push(space.send_notify(k2_agent, no_from_agent, req));
            }

            timing_trace_out!(
                async move {
                    for r in futures::future::join_all(all).await {
                        if let Err(err) = r {
                            tracing::debug!(?err, "send_remote_signal failed");
                        }
                    }

                    Ok(())
                },
                byte_count,
                a = "send_remote_signal"
            )
        })
    }

    fn publish(
        &self,
        _dna_hash: DnaHash,
        _request_validation_receipt: bool,
        _countersigning_session: bool,
        _basis_hash: holo_hash::OpBasis,
        _source: AgentPubKey,
        _op_hash_list: Vec<DhtOpHash>,
        _timeout_ms: Option<u64>,
        _reflect_ops: Option<Vec<DhtOp>>,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn publish_countersign(
        &self,
        _dna_hash: DnaHash,
        _flag: bool,
        _basis_hash: holo_hash::OpBasis,
        _op: DhtOp,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn get(
        &self,
        _dna_hash: DnaHash,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: actor::GetOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireOps>>> {
        Box::pin(async move { todo!() })
    }

    fn get_meta(
        &self,
        _dna_hash: DnaHash,
        _dht_hash: holo_hash::AnyDhtHash,
        _options: actor::GetMetaOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MetadataSet>>> {
        Box::pin(async move { todo!() })
    }

    fn get_links(
        &self,
        _dna_hash: DnaHash,
        _link_key: WireLinkKey,
        _options: actor::GetLinksOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<WireLinkOps>>> {
        Box::pin(async move { todo!() })
    }

    fn count_links(
        &self,
        _dna_hash: DnaHash,
        _query: WireLinkQuery,
    ) -> BoxFut<'_, HolochainP2pResult<CountLinksResponse>> {
        Box::pin(async move { todo!() })
    }

    fn get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: actor::GetActivityOptions,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<AgentActivityResponse>>> {
        Box::pin(async move { todo!() })
    }

    fn must_get_agent_activity(
        &self,
        _dna_hash: DnaHash,
        _author: AgentPubKey,
        _filter: holochain_zome_types::chain::ChainFilter,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<MustGetAgentActivityResponse>>> {
        Box::pin(async move { todo!() })
    }

    fn send_validation_receipts(
        &self,
        _dna_hash: DnaHash,
        _to_agent: AgentPubKey,
        _receipts: ValidationReceiptBundle,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn new_integrated_data(&self, _dna_hash: DnaHash) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn authority_for_hash(
        &self,
        _dna_hash: DnaHash,
        _basis: OpBasis,
    ) -> BoxFut<'_, HolochainP2pResult<bool>> {
        Box::pin(async move { todo!() })
    }

    fn countersigning_session_negotiation(
        &self,
        _dna_hash: DnaHash,
        _agents: Vec<AgentPubKey>,
        _message: event::CountersigningSessionNegotiationMessage,
    ) -> BoxFut<'_, HolochainP2pResult<()>> {
        Box::pin(async move { todo!() })
    }

    fn dump_network_metrics(
        &self,
        _dna_hash: Option<DnaHash>,
    ) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async move { todo!() })
    }

    fn dump_network_stats(&self) -> BoxFut<'_, HolochainP2pResult<String>> {
        Box::pin(async move { todo!() })
    }

    /*
    fn get_diagnostics(&self, _dna_hash: DnaHash) -> BoxFut<'_, HolochainP2pResult<KitsuneDiagnostics>>
    {
        Box::pin(async move { todo!() })
    }
    */

    fn storage_arcs(
        &self,
        _dna_hash: DnaHash,
    ) -> BoxFut<'_, HolochainP2pResult<Vec<kitsune2_api::DhtArc>>> {
        Box::pin(async move { todo!() })
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
        tracing::instrument(skip(self, dna_hash, dht_hash, options), level = "trace")
    )]
    fn handle_get(
        &mut self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireOps>> {
        let space = dna_hash.into_kitsune();
        let basis = dht_hash.to_kitsune();
        let r_options: event::GetOptions = (&options).into();

        let payload = crate::wire::WireMessage::get(dht_hash, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                let result = kitsune_p2p
                    .rpc_multi(input)
                    .instrument(tracing::debug_span!("rpc_multi"))
                    .await?;

                let mut out = Vec::new();
                for item in result {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                    out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
                }

                Ok(out)
            },
            a = "send_get"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_get_meta(
        &mut self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pHandlerResult<Vec<MetadataSet>> {
        let space = dna_hash.into_kitsune();
        let basis = dht_hash.to_kitsune();
        let r_options: event::GetMetaOptions = (&options).into();

        let payload = crate::wire::WireMessage::get_meta(dht_hash, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                let result = kitsune_p2p.rpc_multi(input).await?;

                let mut out = Vec::new();
                for item in result {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                    out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
                }

                Ok(out)
            },
            a = "send_get_meta"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_get_links(
        &mut self,
        dna_hash: DnaHash,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireLinkOps>> {
        let space = dna_hash.into_kitsune();
        let basis = link_key.base.to_kitsune();
        let r_options: event::GetLinksOptions = (&options).into();

        let payload = crate::wire::WireMessage::get_links(link_key, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let mut input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                // NOTE - We're just targeting a single remote node for now
                //        without doing any pagination / etc...
                //        Setting up RpcMulti to act like RpcSingle
                input.max_remote_agent_count = 1;
                let result = kitsune_p2p.rpc_multi(input).await?;

                let mut out = Vec::new();
                for item in result {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                    out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
                }

                Ok(out)
            },
            a = "send_get_links"
        )
    }

    fn handle_count_links(
        &mut self,
        dna_hash: DnaHash,
        query: WireLinkQuery,
    ) -> HolochainP2pHandlerResult<CountLinksResponse> {
        let space = dna_hash.into_kitsune();
        let basis = query.base.to_kitsune();

        let payload = WireMessage::count_links(query).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let mut input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                input.max_remote_agent_count = 1;
                let result = kitsune_p2p.rpc_multi(input).await?;

                if let Some(result) = result.into_iter().next() {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = result;
                    Ok(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?)
                } else {
                    Err(HolochainP2pError::from(
                        "Failed to fetch link count from a peer",
                    ))
                }
            },
            a = "send_count_links"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pHandlerResult<Vec<AgentActivityResponse>> {
        let space = dna_hash.into_kitsune();
        // Convert the agent key to an any dht hash so that it can be used
        // as the basis for sending this request
        let agent_hash: AnyDhtHash = agent.clone().into();
        let basis = agent_hash.to_kitsune();
        let r_options: event::GetActivityOptions = (&options).into();

        let payload =
            crate::wire::WireMessage::get_agent_activity(agent, query, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let mut input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                // TODO - We're just targeting a single remote node for now
                //        without doing any pagination / etc...
                //        Setting up RpcMulti to act like RpcSingle
                input.max_remote_agent_count = 1;
                let result = kitsune_p2p.rpc_multi(input).await?;

                let mut out = Vec::new();
                for item in result {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                    out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
                }

                Ok(out)
            },
            a = "send_get_agent_activity"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_must_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> HolochainP2pHandlerResult<Vec<MustGetAgentActivityResponse>> {
        let space = dna_hash.into_kitsune();
        // Convert the agent key to an any dht hash so it can be used
        // as the basis for sending this request
        let agent_hash: AnyDhtHash = agent.clone().into();
        let basis = agent_hash.to_kitsune();

        let payload = crate::wire::WireMessage::must_get_agent_activity(agent, filter).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        let tuning_params = self.config.tuning_params.clone();
        timing_trace_out!(
            async move {
                let mut input =
                    kitsune_p2p::actor::RpcMulti::new(&tuning_params, space, basis, payload);
                // TODO - We're just targeting a single remote node for now
                //        without doing any pagination / etc...
                //        Setting up RpcMulti to act like RpcSingle
                input.max_remote_agent_count = 1;
                let result = kitsune_p2p.rpc_multi(input).await?;

                let mut out = Vec::new();
                for item in result {
                    let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                    out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
                }

                Ok(out)
            },
            a = "send_must_get_agent_activity"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_send_validation_receipts(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let to_agent = to_agent.into_kitsune();

        let req = crate::wire::WireMessage::validation_receipts(receipts).encode()?;

        let timeout = self.config.tuning_params.implicit_timeout();

        let kitsune_p2p = self.kitsune_p2p.clone();
        timing_trace_out!(
            async move {
                kitsune_p2p
                    .targeted_broadcast(space, vec![to_agent], timeout, req, false)
                    .await?;
                Ok(())
            },
            a = "send_validation_receipts"
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_new_integrated_data(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(
            async move { Ok(kitsune_p2p.new_integrated_data(space).await?) }
                .boxed()
                .into(),
        )
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_authority_for_hash(
        &mut self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
    ) -> HolochainP2pHandlerResult<bool> {
        let space = dna_hash.into_kitsune();
        let basis = basis_hash.to_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(
            async move { Ok(kitsune_p2p.authority_for_hash(space, basis).await?) }
                .boxed()
                .into(),
        )
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

    fn handle_storage_arcs(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<Vec<DhtArc>> {
        let space = dna_hash.into_kitsune();
        let kitsune_p2p = self.kitsune_p2p.clone();

        Ok(async move {
            kitsune_p2p
                .storage_arcs(space)
                .await
                .map_err(HolochainP2pError::other)
        }
        .boxed()
        .into())
    }
    ----------------- */
}
