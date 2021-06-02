#![allow(clippy::too_many_arguments)]
use crate::actor::*;
use crate::event::*;
use crate::*;

use futures::future::FutureExt;

use crate::types::AgentPubKeyExt;

use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;

use holochain_zome_types::zome::FunctionName;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::agent_store::AgentInfoSigned;
use std::future::Future;

macro_rules! timing_trace {
    ($code:block $($rest:tt)*) => {{
        let __start = std::time::Instant::now();
        let __out = $code;
        async move {
            let __out = __out.await;
            let __elapsed_s = __start.elapsed().as_secs_f64();
            if __elapsed_s >= 5.0 {
                tracing::warn!( elapsed_s = %__elapsed_s $($rest)* );
            } else {
                tracing::debug!( elapsed_s = %__elapsed_s $($rest)* );
            }
            __out
        }
    }};
}

#[derive(Clone)]
struct WrapEvtSender(futures::channel::mpsc::Sender<HolochainP2pEvent>);

impl WrapEvtSender {
    pub fn put_agent_info_signed(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent_info_signed: AgentInfoSigned,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .put_agent_info_signed(dna_hash, to_agent, agent_info_signed)
            },
            "(hp2p:handle) put_agent_info_signed",
        )
    }

    fn get_agent_info_signed(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
    ) -> impl Future<Output = HolochainP2pResult<Option<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .get_agent_info_signed(dna_hash, to_agent, kitsune_space, kitsune_agent)
            },
            "(hp2p:handle) get_agent_info_signed",
        )
    }

    fn query_agent_info_signed(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        kitsune_agent: Arc<kitsune_p2p::KitsuneAgent>,
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .query_agent_info_signed(dna_hash, to_agent, kitsune_space, kitsune_agent)
            },
            "(hp2p:handle) query_agent_info_signed",
        )
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> impl Future<Output = HolochainP2pResult<SerializedBytes>> + 'static + Send {
        timing_trace!(
            {
                self.0.call_remote(
                    dna_hash, to_agent, from_agent, zome_name, fn_name, cap, payload,
                )
            },
            "(hp2p:handle) call_remote",
        )
    }

    fn publish(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        let op_count = ops.len();
        timing_trace!({
            self.0.publish(dna_hash, to_agent, from_agent, request_validation_receipt, dht_hash, ops)
        }, %op_count, "(hp2p:handle) publish")
    }

    fn get_validation_package(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        header_hash: HeaderHash,
    ) -> impl Future<Output = HolochainP2pResult<ValidationPackageResponse>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .get_validation_package(dna_hash, to_agent, header_hash)
            },
            "(hp2p:handle) get_validation_package",
        )
    }

    fn get(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetOptions,
    ) -> impl Future<Output = HolochainP2pResult<WireOps>> + 'static + Send {
        timing_trace!(
            { self.0.get(dna_hash, to_agent, dht_hash, options) },
            "(hp2p:handle) get",
        )
    }

    fn get_meta(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: event::GetMetaOptions,
    ) -> impl Future<Output = HolochainP2pResult<MetadataSet>> + 'static + Send {
        timing_trace!(
            { self.0.get_meta(dna_hash, to_agent, dht_hash, options) },
            "(hp2p:handle) get_meta",
        )
    }

    fn get_links(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: event::GetLinksOptions,
    ) -> impl Future<Output = HolochainP2pResult<WireLinkOps>> + 'static + Send {
        timing_trace!(
            { self.0.get_links(dna_hash, to_agent, link_key, options) },
            "(hp2p:handle) get_links",
        )
    }

    fn get_agent_activity(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: event::GetActivityOptions,
    ) -> impl Future<Output = HolochainP2pResult<AgentActivityResponse<HeaderHash>>> + 'static + Send
    {
        timing_trace!(
            {
                self.0
                    .get_agent_activity(dna_hash, to_agent, agent, query, options)
            },
            "(hp2p:handle) get_agent_activity",
        )
    }

    fn validation_receipt_received(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipt: SerializedBytes,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .validation_receipt_received(dna_hash, to_agent, receipt)
            },
            "(hp2p:handle) validation_receipt_received",
        )
    }

    fn fetch_op_hashes_for_constraints(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        dht_arc: kitsune_p2p::dht_arc::DhtArc,
        since: holochain_types::Timestamp,
        until: holochain_types::Timestamp,
    ) -> impl Future<Output = HolochainP2pResult<Vec<holo_hash::DhtOpHash>>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .fetch_op_hashes_for_constraints(dna_hash, to_agent, dht_arc, since, until)
            },
            "(hp2p:handle) fetch_op_hashes_for_constraints",
        )
    }

    fn fetch_op_hash_data(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        op_hashes: Vec<holo_hash::DhtOpHash>,
    ) -> impl Future<
        Output = HolochainP2pResult<
            Vec<(
                holo_hash::AnyDhtHash,
                holo_hash::DhtOpHash,
                holochain_types::dht_op::DhtOp,
            )>,
        >,
    >
           + 'static
           + Send {
        let op_count = op_hashes.len();
        timing_trace!(
            { self.0.fetch_op_hash_data(dna_hash, to_agent, op_hashes) },
            %op_count,
            "(hp2p:handle) fetch_op_hash_data",
        )
    }

    fn sign_network_data(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        data: Vec<u8>,
    ) -> impl Future<Output = HolochainP2pResult<Signature>> + 'static + Send {
        let byte_count = data.len();
        timing_trace!(
            { self.0.sign_network_data(dna_hash, to_agent, data) },
            %byte_count,
            "(hp2p:handle) sign_network_data",
        )
    }
}

pub(crate) struct HolochainP2pActor {
    evt_sender: WrapEvtSender,
    kitsune_p2p: ghost_actor::GhostSender<kitsune_p2p::actor::KitsuneP2p>,
}

impl ghost_actor::GhostControlHandler for HolochainP2pActor {}

impl HolochainP2pActor {
    /// constructor
    pub async fn new(
        config: kitsune_p2p::KitsuneP2pConfig,
        tls_config: kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
    ) -> HolochainP2pResult<Self> {
        let (kitsune_p2p, kitsune_p2p_events) =
            kitsune_p2p::spawn_kitsune_p2p(config, tls_config).await?;

        channel_factory.attach_receiver(kitsune_p2p_events).await?;

        Ok(Self {
            evt_sender: WrapEvtSender(evt_sender),
            kitsune_p2p,
        })
    }

    /// receiving an incoming request from a remote node
    #[allow(clippy::too_many_arguments)]
    fn handle_incoming_call_remote(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        data: Vec<u8>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .call_remote(
                    dna_hash,
                    to_agent,
                    from_agent,
                    zome_name,
                    fn_name,
                    cap,
                    ExternIO::from(data),
                )
                .await;
            res.map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }

    /// receiving an incoming get request from a remote node
    #[tracing::instrument(skip(self, dna_hash, to_agent, dht_hash, options), level = "trace")]
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

    /// receiving an incoming publish from a remote node
    fn handle_incoming_publish(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .publish(
                    dna_hash,
                    to_agent,
                    from_agent,
                    request_validation_receipt,
                    dht_hash,
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
        receipt: Vec<u8>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let receipt: SerializedBytes = UnsafeBytes::from(receipt).into();
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .validation_receipt_received(dna_hash, agent_pub_key, receipt)
                .await?;

            // validation receipts don't need a response
            // send back an empty vec for now
            Ok(Vec::with_capacity(0))
        }
        .boxed()
        .into())
    }

    /// Receiving an incoming validation package request
    fn handle_incoming_get_validation_package(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        header_hash: HeaderHash,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<Vec<u8>> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let res = evt_sender
                .get_validation_package(dna_hash, agent_pub_key, header_hash)
                .await;

            res.and_then(|r| Ok(SerializedBytes::try_from(r)?))
                .map_err(kitsune_p2p::KitsuneP2pError::from)
                .map(|res| UnsafeBytes::from(res).into())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<kitsune_p2p::event::KitsuneP2pEvent> for HolochainP2pActor {}

impl kitsune_p2p::event::KitsuneP2pEventHandler for HolochainP2pActor {
    /// We need to store signed agent info.
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_put_agent_info_signed(
        &mut self,
        input: kitsune_p2p::event::PutAgentInfoSignedEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let kitsune_p2p::event::PutAgentInfoSignedEvt {
            space,
            agent,
            agent_info_signed,
        } = input;
        let space = DnaHash::from_kitsune(&space);
        let agent = AgentPubKey::from_kitsune(&agent);
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .put_agent_info_signed(space, agent, agent_info_signed)
                .await?)
        }
        .boxed()
        .into())
    }

    /// We need to get previously stored agent info.
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_get_agent_info_signed(
        &mut self,
        input: kitsune_p2p::event::GetAgentInfoSignedEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Option<AgentInfoSigned>> {
        let kitsune_p2p::event::GetAgentInfoSignedEvt { space, agent } = input;
        let h_space = DnaHash::from_kitsune(&space);
        let h_agent = AgentPubKey::from_kitsune(&agent);
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .get_agent_info_signed(h_space, h_agent, space, agent)
                .await?)
        }
        .boxed()
        .into())
    }

    /// We need to get previously stored agent info.
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_query_agent_info_signed(
        &mut self,
        input: kitsune_p2p::event::QueryAgentInfoSignedEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        let kitsune_p2p::event::QueryAgentInfoSignedEvt { space, agent } = input;
        let h_space = DnaHash::from_kitsune(&space);
        let h_agent = AgentPubKey::from_kitsune(&agent);
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .query_agent_info_signed(h_space, h_agent, space, agent)
                .await?)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_query_agent_info_signed_near_basis(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        basis_loc: u32,
        limit: u32,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        let h_space = DnaHash::from_kitsune(&space);
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .query_agent_info_signed_near_basis(h_space, space, basis_loc, limit)
                .await?)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self, space, to_agent, from_agent, payload), level = "trace")]
    fn handle_call(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        from_agent: Arc<kitsune_p2p::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<u8>> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);
        let from_agent = AgentPubKey::from_kitsune(&from_agent);

        let request =
            crate::wire::WireMessage::decode(payload.as_ref()).map_err(HolochainP2pError::from)?;

        match request {
            crate::wire::WireMessage::CallRemote {
                zome_name,
                fn_name,
                cap,
                data,
            } => self.handle_incoming_call_remote(
                space, to_agent, from_agent, zome_name, fn_name, cap, data,
            ),
            crate::wire::WireMessage::Get { dht_hash, options } => {
                self.handle_incoming_get(space, to_agent, dht_hash, options)
            }
            crate::wire::WireMessage::GetMeta { dht_hash, options } => {
                self.handle_incoming_get_meta(space, to_agent, dht_hash, options)
            }
            crate::wire::WireMessage::GetLinks { link_key, options } => {
                self.handle_incoming_get_links(space, to_agent, link_key, options)
            }
            crate::wire::WireMessage::GetAgentActivity {
                agent,
                query,
                options,
            } => self.handle_incoming_get_agent_activity(space, to_agent, agent, query, options),
            // holochain_p2p never publishes via request
            // these only occur on broadcasts
            crate::wire::WireMessage::Publish { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid: publish is a broadcast type, not a request".to_string(),
                )
                .into())
            }
            crate::wire::WireMessage::ValidationReceipt { receipt } => {
                self.handle_incoming_validation_receipt(space, to_agent, receipt)
            }
            crate::wire::WireMessage::GetValidationPackage { header_hash } => {
                self.handle_incoming_get_validation_package(space, to_agent, header_hash)
            }
        }
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_notify(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        from_agent: Arc<kitsune_p2p::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);
        let from_agent = AgentPubKey::from_kitsune(&from_agent);

        let request =
            crate::wire::WireMessage::decode(payload.as_ref()).map_err(HolochainP2pError::from)?;

        match request {
            // error on these call type messages
            crate::wire::WireMessage::CallRemote { .. }
            | crate::wire::WireMessage::Get { .. }
            | crate::wire::WireMessage::GetMeta { .. }
            | crate::wire::WireMessage::GetLinks { .. }
            | crate::wire::WireMessage::GetAgentActivity { .. }
            | crate::wire::WireMessage::GetValidationPackage { .. }
            | crate::wire::WireMessage::ValidationReceipt { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid call type message in a notify".to_string(),
                )
                .into())
            }
            crate::wire::WireMessage::Publish {
                request_validation_receipt,
                dht_hash,
                ops,
            } => self.handle_incoming_publish(
                space,
                to_agent,
                from_agent,
                request_validation_receipt,
                dht_hash,
                ops,
            ),
        }
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_gossip(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        from_agent: Arc<kitsune_p2p::KitsuneAgent>,
        op_hash: Arc<kitsune_p2p::KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);
        let _from_agent = AgentPubKey::from_kitsune(&from_agent);
        let op_hash = DhtOpHash::from_kitsune(&op_hash);
        let op_data =
            crate::wire::WireDhtOpData::decode(op_data).map_err(HolochainP2pError::from)?;
        self.handle_incoming_publish(
            space,
            to_agent,
            op_data.from_agent,
            false,
            op_data.dht_hash,
            vec![(op_hash, op_data.op_data)],
        )
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        input: kitsune_p2p::event::FetchOpHashesForConstraintsEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<Arc<kitsune_p2p::KitsuneOpHash>>>
    {
        let kitsune_p2p::event::FetchOpHashesForConstraintsEvt {
            space,
            agent,
            dht_arc,
            since_utc_epoch_s,
            until_utc_epoch_s,
        } = input;
        let space = DnaHash::from_kitsune(&space);
        let agent = AgentPubKey::from_kitsune(&agent);
        let since = Timestamp(since_utc_epoch_s, 0);
        let until = Timestamp(until_utc_epoch_s, 0);

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .fetch_op_hashes_for_constraints(space, agent, dht_arc, since, until)
                .await?
                .into_iter()
                .map(|h| h.into_kitsune())
                .collect())
        }
        .boxed()
        .into())
    }

    #[allow(clippy::needless_collect)]
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_fetch_op_hash_data(
        &mut self,
        input: kitsune_p2p::event::FetchOpHashDataEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<
        Vec<(Arc<kitsune_p2p::KitsuneOpHash>, Vec<u8>)>,
    > {
        let kitsune_p2p::event::FetchOpHashDataEvt {
            space,
            agent,
            op_hashes,
        } = input;
        let space = DnaHash::from_kitsune(&space);
        let agent = AgentPubKey::from_kitsune(&agent);
        let op_hashes = op_hashes
            .into_iter()
            .map(|h| DhtOpHash::from_kitsune(&h))
            // the allowance of clippy::needless_collcect refers to the following call
            .collect::<Vec<_>>();

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let mut out = vec![];
            for (dht_hash, op_hash, dht_op) in evt_sender
                .fetch_op_hash_data(space, agent.clone(), op_hashes)
                .await?
            {
                out.push((
                    op_hash.into_kitsune(),
                    crate::wire::WireDhtOpData {
                        from_agent: agent.clone(),
                        dht_hash,
                        op_data: dht_op,
                    }
                    .encode()
                    .map_err(kitsune_p2p::KitsuneP2pError::other)?,
                ));
            }
            Ok(out)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_sign_network_data(
        &mut self,
        input: kitsune_p2p::event::SignNetworkDataEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<kitsune_p2p::KitsuneSignature> {
        let space = DnaHash::from_kitsune(&input.space);
        let agent = AgentPubKey::from_kitsune(&input.agent);
        let fut = self
            .evt_sender
            .sign_network_data(space, agent, input.data.to_vec());
        Ok(async move {
            let sig = fut.await?.0;
            Ok(sig.to_vec().into())
        }
        .boxed()
        .into())
    }
}

impl ghost_actor::GhostHandler<HolochainP2p> for HolochainP2pActor {}

impl HolochainP2pHandler for HolochainP2pActor {
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_join(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move { Ok(kitsune_p2p.join(space, agent).await?) }
            .boxed()
            .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_leave(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move { Ok(kitsune_p2p.leave(space, agent).await?) }
            .boxed()
            .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_call_remote(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        let space = dna_hash.into_kitsune();
        let to_agent = to_agent.into_kitsune();
        let from_agent = from_agent.into_kitsune();

        let req =
            crate::wire::WireMessage::call_remote(zome_name, fn_name, cap, payload).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let result: Vec<u8> = kitsune_p2p
                .rpc_single(space, to_agent, from_agent, req, None)
                .await?;
            Ok(UnsafeBytes::from(result).into())
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_publish(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        let basis = dht_hash.to_kitsune();

        let payload = crate::wire::WireMessage::publish(request_validation_receipt, dht_hash, ops)
            .encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .notify_multi(kitsune_p2p::actor::NotifyMulti {
                    space,
                    from_agent,
                    basis,
                    remote_agent_count: None, // default best-effort
                    timeout_ms,
                    payload,
                })
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_get_validation_package(
        &mut self,
        input: actor::GetValidationPackage,
    ) -> HolochainP2pHandlerResult<ValidationPackageResponse> {
        let space = input.dna_hash.into_kitsune();
        let to_agent = input.request_from.into_kitsune();
        let from_agent = input.agent_pub_key.into_kitsune();

        let req = crate::wire::WireMessage::get_validation_package(input.header_hash).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let response = kitsune_p2p
                .rpc_single(space, to_agent, from_agent, req, None)
                .await?;
            let response = SerializedBytes::from(UnsafeBytes::from(response)).try_into()?;
            Ok(response)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self, dna_hash, from_agent, dht_hash, options), level = "trace")]
    fn handle_get(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireOps>> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        let basis = dht_hash.to_kitsune();
        let r_options: event::GetOptions = (&options).into();

        let payload = crate::wire::WireMessage::get(dht_hash, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let result = kitsune_p2p
                .rpc_multi(kitsune_p2p::actor::RpcMulti {
                    space,
                    from_agent,
                    basis,
                    remote_agent_count: options.remote_agent_count,
                    timeout_ms: options.timeout_ms,
                    as_race: options.as_race,
                    race_timeout_ms: options.race_timeout_ms,
                    payload,
                })
                .instrument(tracing::debug_span!("rpc_multi"))
                .await?;

            let mut out = Vec::new();
            for item in result {
                let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
            }

            Ok(out)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_get_meta(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pHandlerResult<Vec<MetadataSet>> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        let basis = dht_hash.to_kitsune();
        let r_options: event::GetMetaOptions = (&options).into();

        let payload = crate::wire::WireMessage::get_meta(dht_hash, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            let result = kitsune_p2p
                .rpc_multi(kitsune_p2p::actor::RpcMulti {
                    space,
                    from_agent,
                    basis,
                    remote_agent_count: options.remote_agent_count,
                    timeout_ms: options.timeout_ms,
                    as_race: options.as_race,
                    race_timeout_ms: options.race_timeout_ms,
                    payload,
                })
                .await?;

            let mut out = Vec::new();
            for item in result {
                let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
            }

            Ok(out)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_get_links(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireLinkOps>> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        let basis = AnyDhtHash::from(link_key.base.clone()).to_kitsune();
        let r_options: event::GetLinksOptions = (&options).into();

        let payload = crate::wire::WireMessage::get_links(link_key, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            // TODO - We're just targeting a single remote node for now
            //        without doing any pagination / etc...
            //        Setting up RpcMulti to act like RpcSingle
            let result = kitsune_p2p
                .rpc_multi(kitsune_p2p::actor::RpcMulti {
                    space,
                    from_agent,
                    basis,
                    remote_agent_count: Some(1),
                    timeout_ms: options.timeout_ms,
                    as_race: false,
                    race_timeout_ms: options.timeout_ms,
                    payload,
                })
                .await?;

            let mut out = Vec::new();
            for item in result {
                let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
            }

            Ok(out)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pHandlerResult<Vec<AgentActivityResponse<HeaderHash>>> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        // Convert the agent key to an any dht hash so it can be used
        // as the basis for sending this request
        let agent_hash: AnyDhtHash = agent.clone().into();
        let basis = agent_hash.to_kitsune();
        let r_options: event::GetActivityOptions = (&options).into();

        let payload =
            crate::wire::WireMessage::get_agent_activity(agent, query, r_options).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            // TODO - We're just targeting a single remote node for now
            //        without doing any pagination / etc...
            //        Setting up RpcMulti to act like RpcSingle
            let result = kitsune_p2p
                .rpc_multi(kitsune_p2p::actor::RpcMulti {
                    space,
                    from_agent,
                    basis,
                    remote_agent_count: Some(1),
                    timeout_ms: options.timeout_ms,
                    as_race: false,
                    race_timeout_ms: options.timeout_ms,
                    payload,
                })
                .await?;

            let mut out = Vec::new();
            for item in result {
                let kitsune_p2p::actor::RpcMultiResponse { response, .. } = item;
                out.push(SerializedBytes::from(UnsafeBytes::from(response)).try_into()?);
            }

            Ok(out)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_send_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        receipt: SerializedBytes,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let to_agent = to_agent.into_kitsune();
        let from_agent = from_agent.into_kitsune();

        let req = crate::wire::WireMessage::validation_receipt(receipt).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .rpc_single(space, to_agent, from_agent, req, None)
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }
}
