#![allow(clippy::too_many_arguments)]
use crate::actor::*;
use crate::event::*;
use crate::*;

use futures::future::FutureExt;
use kitsune_p2p::event::full_time_window;
use kitsune_p2p::event::MetricDatum;
use kitsune_p2p::event::MetricKind;
use kitsune_p2p::event::MetricQuery;
use kitsune_p2p::event::MetricQueryAnswer;
use kitsune_p2p::event::TimeWindow;

use crate::types::AgentPubKeyExt;

use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;

use holochain_zome_types::zome::FunctionName;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::agent_store::AgentInfoSigned;
use std::collections::HashSet;
use std::future::Future;
use std::time::SystemTime;

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
                tracing::trace!( elapsed_s = %__elapsed_s $($rest)* );
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
        peer_data: Vec<AgentInfoSigned>,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            { self.0.put_agent_info_signed(dna_hash, peer_data) },
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

    fn query_gossip_agents(
        &self,
        dna_hash: DnaHash,
        agents: Option<Vec<AgentPubKey>>,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        since_ms: u64,
        until_ms: u64,
        arc_set: Arc<kitsune_p2p_types::dht_arc::DhtArcSet>,
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            {
                self.0.query_gossip_agents(
                    dna_hash,
                    agents,
                    kitsune_space,
                    since_ms,
                    until_ms,
                    arc_set,
                )
            },
            "(hp2p:handle) query_gossip_agents",
        )
    }

    fn query_agent_info_signed(
        &self,
        dna_hash: DnaHash,
        agents: Option<HashSet<Arc<kitsune_p2p::KitsuneAgent>>>,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .query_agent_info_signed(dna_hash, agents, kitsune_space)
            },
            "(hp2p:handle) query_agent_info_signed",
        )
    }

    fn query_agent_info_signed_near_basis(
        &self,
        dna_hash: DnaHash,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        basis_loc: u32,
        limit: u32,
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .query_agent_info_signed_near_basis(dna_hash, kitsune_space, basis_loc, limit)
            },
            "(hp2p:handle) query_agent_info_signed_near_basis",
        )
    }

    fn query_peer_density(
        &self,
        dna_hash: DnaHash,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> impl Future<Output = HolochainP2pResult<kitsune_p2p_types::dht_arc::PeerDensity>> + 'static + Send
    {
        timing_trace!(
            { self.0.query_peer_density(dna_hash, kitsune_space, dht_arc) },
            "(hp2p:handle) query_peer_density",
        )
    }

    fn put_metric_datum(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        agent: AgentPubKey,
        kind: MetricKind,
        timestamp: SystemTime,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .put_metric_datum(dna_hash, to_agent, agent, kind, timestamp)
            },
            "(hp2p:handle) put_metric_datum",
        )
    }

    fn query_metrics(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        query: MetricQuery,
    ) -> impl Future<Output = HolochainP2pResult<MetricQueryAnswer>> + 'static + Send {
        timing_trace!(
            { self.0.query_metrics(dna_hash, to_agent, query) },
            "(hp2p:handle) query_metrics",
        )
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
    ) -> impl Future<Output = HolochainP2pResult<SerializedBytes>> + 'static + Send {
        timing_trace!(
            {
                self.0.call_remote(
                    dna_hash, to_agent, from_agent, zome_name, fn_name, cap_secret, payload,
                )
            },
            "(hp2p:handle) call_remote",
        )
    }

    fn publish(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        request_validation_receipt: bool,
        countersigning_session: bool,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        let op_count = ops.len();
        timing_trace!({
            self.0.publish(dna_hash, to_agent, request_validation_receipt, countersigning_session, ops)
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

    fn query_op_hashes(
        &self,
        dna_hash: DnaHash,
        agents: Vec<(AgentPubKey, kitsune_p2p::dht_arc::DhtArcSet)>,
        window: TimeWindow,
        max_ops: usize,
        include_limbo: bool,
    ) -> impl Future<Output = HolochainP2pResult<Option<(Vec<holo_hash::DhtOpHash>, TimeWindow)>>>
           + 'static
           + Send {
        timing_trace!(
            {
                self.0
                    .query_op_hashes(dna_hash, agents, window, max_ops, include_limbo)
            },
            "(hp2p:handle) query_op_hashes",
        )
    }

    fn fetch_op_data(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        op_hashes: Vec<holo_hash::DhtOpHash>,
    ) -> impl Future<
        Output = HolochainP2pResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>>,
    >
           + 'static
           + Send {
        let op_count = op_hashes.len();
        timing_trace!(
            { self.0.fetch_op_data(dna_hash, to_agent, op_hashes) },
            %op_count,
            "(hp2p:handle) fetch_op_data",
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

    fn countersigning_authority_response(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        signed_headers: Vec<SignedHeader>,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            {
                self.0
                    .countersigning_authority_response(dna_hash, to_agent, signed_headers)
            },
            "(hp2p:handle) signed_header"
        )
    }
}

pub(crate) struct HolochainP2pActor {
    tuning_params: kitsune_p2p_types::config::KitsuneP2pTuningParams,
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
        let tuning_params = config.tuning_params.clone();
        let (kitsune_p2p, kitsune_p2p_events) =
            kitsune_p2p::spawn_kitsune_p2p(config, tls_config).await?;

        channel_factory.attach_receiver(kitsune_p2p_events).await?;

        Ok(Self {
            tuning_params,
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
        cap_secret: Option<CapSecret>,
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
                    cap_secret,
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
        request_validation_receipt: bool,
        countersigning_session: bool,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .publish(
                    dna_hash,
                    to_agent,
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

    fn handle_incoming_countersigning_authority_response(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        signed_headers: Vec<SignedHeader>,
    ) -> kitsune_p2p::actor::KitsuneP2pHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            evt_sender
                .countersigning_authority_response(dna_hash, to_agent, signed_headers)
                .await?;
            Ok(())
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
        let kitsune_p2p::event::PutAgentInfoSignedEvt { space, peer_data } = input;
        let space = DnaHash::from_kitsune(&space);
        let evt_sender = self.evt_sender.clone();
        Ok(
            async move { Ok(evt_sender.put_agent_info_signed(space, peer_data).await?) }
                .boxed()
                .into(),
        )
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

    /// We need to get previously stored agent info. A single kitusne agent query
    /// can take one of three Holochain agent query paths. We do "duck typing"
    /// on the query object to determine which query path to take. The reason for
    /// this is that Holochain is optimized for these three query types, while
    /// kitsune has a more general interface.
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_query_agents(
        &mut self,
        input: kitsune_p2p::event::QueryAgentsEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        let kitsune_p2p::event::QueryAgentsEvt {
            space,
            agents,
            window,
            arc_set,
            near_basis,
            limit,
        } = input;

        let h_space = DnaHash::from_kitsune(&space);
        let evt_sender = self.evt_sender.clone();

        Ok(async move {
            let agents = match (agents, window, arc_set, near_basis, limit) {
                // If only basis and limit are set, this is a "near basis" query
                (None, None, None, Some(basis), Some(limit)) => {
                    evt_sender
                        .query_agent_info_signed_near_basis(h_space, space, basis.as_u32(), limit)
                        .await?
                }

                // If arc_set is set, this is a "gossip agents" query
                (agents, window, Some(arc_set), None, None) => {
                    let window = window.unwrap_or_else(full_time_window);
                    let h_agents =
                        agents.map(|agents| agents.iter().map(AgentPubKey::from_kitsune).collect());
                    let since_ms = window.start.as_millis().max(0) as u64;
                    let until_ms = window.end.as_millis().max(0) as u64;
                    evt_sender
                        .query_gossip_agents(h_space, h_agents, space, since_ms, until_ms, arc_set)
                        .await?
                }

                // Otherwise, do a simple agent query with optional agent filter
                (agents, None, None, None, None) => {
                    evt_sender
                        .query_agent_info_signed(h_space, agents, space)
                        .await?
                }

                // If none of the above match, we have no implementation for such a query
                // and must fail
                tuple => unimplemented!(
                    "Holochain cannot interpret the QueryAgentsEvt data as given: {:?}",
                    tuple
                ),
            };
            Ok(agents)
        }
        .boxed()
        .into())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_query_peer_density(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerDensity>
    {
        let h_space = DnaHash::from_kitsune(&space);
        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .query_peer_density(h_space, space, dht_arc)
                .await?)
        }
        .boxed()
        .into())
    }

    fn handle_put_metric_datum(
        &mut self,
        datum: MetricDatum,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let evt_sender = self.evt_sender.clone();
        // These dummy values are not used
        let dna_hash = DnaHash::from_raw_32([0; 32].to_vec());
        let to_agent = AgentPubKey::from_raw_32([0; 32].to_vec());

        let agent = AgentPubKey::from_kitsune(&datum.agent);
        let kind = datum.kind;
        let timestamp = datum.timestamp;
        Ok(async move {
            Ok(evt_sender
                .put_metric_datum(dna_hash, to_agent, agent, kind, timestamp)
                .await?)
        }
        .boxed()
        .into())
    }

    fn handle_query_metrics(
        &mut self,
        query: kitsune_p2p::event::MetricQuery,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<MetricQueryAnswer> {
        let evt_sender = self.evt_sender.clone();

        // These dummy values are not used
        let dna_hash = DnaHash::from_raw_32([0; 32].to_vec());
        let to_agent = AgentPubKey::from_raw_32([0; 32].to_vec());

        Ok(
            async move { Ok(evt_sender.query_metrics(dna_hash, to_agent, query).await?) }
                .boxed()
                .into(),
        )
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
                cap_secret,
                data,
            } => self.handle_incoming_call_remote(
                space, to_agent, from_agent, zome_name, fn_name, cap_secret, data,
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
            // holochain_p2p only broadcasts this message.
            crate::wire::WireMessage::CountersigningAuthorityResponse { .. } => {
                Err(HolochainP2pError::invalid_p2p_message(
                    "invalid: countersigning authority response is a broadcast type, not a request"
                        .to_string(),
                )
                .into())
            }
        }
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_notify(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        _from_agent: Arc<kitsune_p2p::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);

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
                countersigning_session,
                dht_hash: _,
                ops,
            } => self.handle_incoming_publish(
                space,
                to_agent,
                request_validation_receipt,
                countersigning_session,
                ops,
            ),
            crate::wire::WireMessage::CountersigningAuthorityResponse { signed_headers } => self
                .handle_incoming_countersigning_authority_response(space, to_agent, signed_headers),
        }
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_gossip(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        to_agent: Arc<kitsune_p2p::KitsuneAgent>,
        ops: Vec<(Arc<kitsune_p2p::KitsuneOpHash>, Vec<u8>)>,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<()> {
        let space = DnaHash::from_kitsune(&space);
        let to_agent = AgentPubKey::from_kitsune(&to_agent);
        let ops = ops
            .into_iter()
            .map(|(op_hash, op_data)| {
                let op_hash = DhtOpHash::from_kitsune(&op_hash);
                let op_data = crate::wire::WireDhtOpData::decode(op_data)
                    .map_err(HolochainP2pError::from)?
                    .op_data;
                Ok((op_hash, op_data))
            })
            .collect::<Result<_, HolochainP2pError>>()?;
        self.handle_incoming_publish(space, to_agent, false, false, ops)
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_query_op_hashes(
        &mut self,
        input: kitsune_p2p::event::QueryOpHashesEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<
        Option<(Vec<Arc<kitsune_p2p::KitsuneOpHash>>, TimeWindow)>,
    > {
        let kitsune_p2p::event::QueryOpHashesEvt {
            space,
            agents,
            window,
            max_ops,
            include_limbo,
        } = input;
        let space = DnaHash::from_kitsune(&space);
        let agents = agents
            .into_iter()
            .map(|(agent, set)| (AgentPubKey::from_kitsune(&agent), set))
            .collect();

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(evt_sender
                .query_op_hashes(space, agents, window, max_ops, include_limbo)
                .await?
                .map(|(h, time)| (h.into_iter().map(|h| h.into_kitsune()).collect(), time)))
        }
        .boxed()
        .into())
    }

    #[allow(clippy::needless_collect)]
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_fetch_op_data(
        &mut self,
        input: kitsune_p2p::event::FetchOpDataEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<
        Vec<(Arc<kitsune_p2p::KitsuneOpHash>, Vec<u8>)>,
    > {
        let kitsune_p2p::event::FetchOpDataEvt {
            space,
            agents,
            op_hashes,
        } = input;
        let space = DnaHash::from_kitsune(&space);
        let agents: Vec<_> = agents.iter().map(AgentPubKey::from_kitsune).collect();
        let op_hashes = op_hashes
            .into_iter()
            .map(|h| DhtOpHash::from_kitsune(&h))
            // the allowance of clippy::needless_collect refers to the following call
            .collect::<Vec<_>>();

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            let mut out = vec![];
            for agent in agents {
                for (op_hash, dht_op) in evt_sender
                    .fetch_op_data(space.clone(), agent.clone(), op_hashes.clone())
                    .await?
                {
                    out.push((
                        op_hash.into_kitsune(),
                        crate::wire::WireDhtOpData { op_data: dht_op }
                            .encode()
                            .map_err(kitsune_p2p::KitsuneP2pError::other)?,
                    ));
                }
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
        cap_secret: Option<CapSecret>,
        payload: ExternIO,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        let space = dna_hash.into_kitsune();
        let to_agent = to_agent.into_kitsune();
        let from_agent = from_agent.into_kitsune();

        let req = crate::wire::WireMessage::call_remote(zome_name, fn_name, cap_secret, payload)
            .encode()?;

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
        _from_agent: AgentPubKey,
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> HolochainP2pHandlerResult<()> {
        use kitsune_p2p_types::KitsuneTimeout;

        let space = dna_hash.into_kitsune();
        let basis = dht_hash.to_kitsune();
        let timeout = match timeout_ms {
            Some(ms) => KitsuneTimeout::from_millis(ms),
            None => self.tuning_params.implicit_timeout(),
        };

        let payload = crate::wire::WireMessage::publish(
            request_validation_receipt,
            countersigning_session,
            dht_hash,
            ops,
        )
        .encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .broadcast(space, basis, timeout, payload)
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
        let tuning_params = self.tuning_params.clone();
        Ok(async move {
            let input = kitsune_p2p::actor::RpcMulti::new(
                &tuning_params,
                space,
                from_agent,
                basis,
                payload,
            );
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
        let tuning_params = self.tuning_params.clone();
        Ok(async move {
            let input = kitsune_p2p::actor::RpcMulti::new(
                &tuning_params,
                space,
                from_agent,
                basis,
                payload,
            );
            let result = kitsune_p2p.rpc_multi(input).await?;

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
        let tuning_params = self.tuning_params.clone();
        Ok(async move {
            let mut input = kitsune_p2p::actor::RpcMulti::new(
                &tuning_params,
                space,
                from_agent,
                basis,
                payload,
            );
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
        let tuning_params = self.tuning_params.clone();
        Ok(async move {
            let mut input = kitsune_p2p::actor::RpcMulti::new(
                &tuning_params,
                space,
                from_agent,
                basis,
                payload,
            );
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

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_new_integrated_data(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(
            async move { Ok(kitsune_p2p.new_integrated_data(space).await?) }
                .boxed()
                .into(),
        )
    }

    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_authority_for_hash(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        dht_hash: AnyDhtHash,
    ) -> HolochainP2pHandlerResult<bool> {
        let space = dna_hash.into_kitsune();
        let agent = agent.into_kitsune();
        let basis = dht_hash.to_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(
            async move { Ok(kitsune_p2p.authority_for_hash(space, agent, basis).await?) }
                .boxed()
                .into(),
        )
    }
    #[tracing::instrument(skip(self), level = "trace")]
    fn handle_countersigning_authority_response(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        agents: Vec<AgentPubKey>,
        response: Vec<SignedHeader>,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let from_agent = from_agent.into_kitsune();
        let agents = agents.into_iter().map(|a| a.into_kitsune()).collect();

        let timeout = self.tuning_params.implicit_timeout();

        let payload =
            crate::wire::WireMessage::countersigning_authority_response(response).encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            kitsune_p2p
                .targeted_broadcast(space, from_agent, agents, timeout, payload)
                .await?;
            Ok(())
        }
        .boxed()
        .into())
    }
}
