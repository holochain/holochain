#![allow(clippy::too_many_arguments)]
use crate::actor::*;
use crate::event::*;
use crate::*;

use futures::future::FutureExt;
use kitsune_p2p::actor::BroadcastData;
use kitsune_p2p::dependencies::kitsune_p2p_fetch;
use kitsune_p2p::dht::Arq;
use kitsune_p2p::event::*;
use kitsune_p2p::gossip::sharded_gossip::KitsuneDiagnostics;
use kitsune_p2p::KOp;
use kitsune_p2p::KitsuneOpData;
use kitsune_p2p::PreflightUserData;
use kitsune_p2p_fetch::FetchContext;

use crate::types::AgentPubKeyExt;

use ghost_actor::dependencies::tracing;
use ghost_actor::dependencies::tracing_futures::Instrument;

use kitsune2_api::DhtArc;
use kitsune_p2p::actor::KitsuneP2pSender;
use kitsune_p2p::agent_store::AgentInfoSigned;
use kitsune_p2p_types::bootstrap::AgentInfoPut;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::iter;

macro_rules! timing_trace {
    ($netaudit:literal, $code:block $($rest:tt)*) => {{
        let __start = std::time::Instant::now();
        let __out = $code;
        async move {
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
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoPut>>> + 'static + Send {
        timing_trace!(
            false,
            { self.0.put_agent_info_signed(dna_hash, peer_data) },
            a = "recv_put_agent_info_signed",
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
            false,
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
            a = "recv_query_gossip_agents",
        )
    }

    fn query_agent_info_signed(
        &self,
        dna_hash: DnaHash,
        agents: Option<HashSet<Arc<kitsune_p2p::KitsuneAgent>>>,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
    ) -> impl Future<Output = HolochainP2pResult<Vec<AgentInfoSigned>>> + 'static + Send {
        timing_trace!(
            false,
            {
                self.0
                    .query_agent_info_signed(dna_hash, agents, kitsune_space)
            },
            a = "recv_query_agent_info_signed",
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
            false,
            {
                self.0
                    .query_agent_info_signed_near_basis(dna_hash, kitsune_space, basis_loc, limit)
            },
            a = "recv_query_agent_info_signed_near_basis",
        )
    }

    fn query_peer_density(
        &self,
        dna_hash: DnaHash,
        kitsune_space: Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> impl Future<Output = HolochainP2pResult<kitsune_p2p_types::dht::PeerView>> + 'static + Send
    {
        timing_trace!(
            false,
            { self.0.query_peer_density(dna_hash, kitsune_space, dht_arc) },
            a = "recv_query_peer_density",
        )
    }

    fn call_remote(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> impl Future<Output = HolochainP2pResult<SerializedBytes>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<WireOps>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<MetadataSet>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<WireLinkOps>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<CountLinksResponse>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<AgentActivityResponse>> + 'static + Send {
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
    ) -> impl Future<Output = HolochainP2pResult<MustGetAgentActivityResponse>> + 'static + Send
    {
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
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
        timing_trace!(
            false,
            {
                self.0
                    .validation_receipts_received(dna_hash, to_agent, receipts)
            },
            a = "recv_validation_receipt_received",
        )
    }

    fn query_op_hashes(
        &self,
        dna_hash: DnaHash,
        arc_set: kitsune_p2p::dht_arc::DhtArcSet,
        window: TimeWindow,
        max_ops: usize,
        include_limbo: bool,
    ) -> impl Future<
        Output = HolochainP2pResult<Option<(Vec<holo_hash::DhtOpHash>, TimeWindowInclusive)>>,
    >
           + 'static
           + Send {
        timing_trace!(
            false,
            {
                self.0
                    .query_op_hashes(dna_hash, arc_set, window, max_ops, include_limbo)
            },
            a = "recv_query_op_hashes",
        )
    }

    fn fetch_op_data(
        &self,
        dna_hash: DnaHash,
        query: FetchOpDataQuery,
    ) -> impl Future<
        Output = HolochainP2pResult<Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>>,
    >
           + 'static
           + Send {
        timing_trace!(
            false,
            { self.0.fetch_op_data(dna_hash, query) },
            a = "recv_fetch_op_data",
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
            false,
            { self.0.sign_network_data(dna_hash, to_agent, data) },
            %byte_count,
            a = "recv_sign_network_data",
        )
    }

    fn countersigning_session_negotiation(
        &self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> impl Future<Output = HolochainP2pResult<()>> + 'static + Send {
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

pub(crate) struct HolochainP2pActor {
    config: kitsune_p2p_types::config::KitsuneP2pConfig,
    evt_sender: WrapEvtSender,
    kitsune_p2p: ghost_actor::GhostSender<kitsune_p2p::actor::KitsuneP2p>,
    host: kitsune_p2p::HostApi,
}

impl ghost_actor::GhostControlHandler for HolochainP2pActor {
    fn handle_ghost_actor_shutdown(
        self,
    ) -> ghost_actor::dependencies::must_future::MustBoxFuture<'static, ()> {
        use ghost_actor::GhostControlSender;
        async move {
            let _ = self.kitsune_p2p.ghost_actor_shutdown_immediate().await;
        }
        .boxed()
        .into()
    }
}

impl HolochainP2pActor {
    /// constructor
    pub async fn new(
        config: kitsune_p2p_types::config::KitsuneP2pConfig,
        tls_config: kitsune_p2p_types::tls::TlsConfig,
        channel_factory: ghost_actor::actor_builder::GhostActorChannelFactory<Self>,
        evt_sender: futures::channel::mpsc::Sender<HolochainP2pEvent>,
        host: kitsune_p2p::HostApi,
        compat: NetworkCompatParams,
    ) -> HolochainP2pResult<Self> {
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

        let (kitsune_p2p, kitsune_p2p_events) = kitsune_p2p::spawn_kitsune_p2p(
            config.clone(),
            tls_config,
            host.clone(),
            preflight_user_data,
        )
        .await?;

        channel_factory.attach_receiver(kitsune_p2p_events).await?;

        Ok(Self {
            config,
            evt_sender: WrapEvtSender(evt_sender),
            kitsune_p2p,
            host,
        })
    }

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
}

impl ghost_actor::GhostHandler<kitsune_p2p::event::KitsuneP2pEvent> for HolochainP2pActor {}

impl kitsune_p2p::event::KitsuneP2pEventHandler for HolochainP2pActor {
    /// We need to store signed agent info.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_put_agent_info_signed(
        &mut self,
        input: kitsune_p2p::event::PutAgentInfoSignedEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<AgentInfoPut>> {
        let kitsune_p2p::event::PutAgentInfoSignedEvt { peer_data } = input;

        let put_requests = peer_data
            .into_iter()
            .map(|agent| (DnaHash::from_kitsune(&agent.space), agent))
            .fold(
                HashMap::<DnaHash, Vec<AgentInfoSigned>>::new(),
                |mut acc, (dna, agent)| {
                    acc.entry(dna).or_default().push(agent);
                    acc
                },
            );

        let evt_sender = self.evt_sender.clone();
        Ok(async move {
            Ok(futures::future::join_all(
                iter::repeat_with(|| evt_sender.clone())
                    .zip(put_requests.into_iter())
                    .map(|(evt_sender, (dna, agents))| async move {
                        evt_sender.put_agent_info_signed(dna, agents).await
                    }),
            )
            .await
            .into_iter()
            .collect::<HolochainP2pResult<Vec<Vec<AgentInfoPut>>>>()?
            .into_iter()
            .flatten()
            .collect())
        }
        .boxed()
        .into())
    }

    /// We need to get previously stored agent info. A single kitusne agent query
    /// can take one of three Holochain agent query paths. We do "duck typing"
    /// on the query object to determine which query path to take. The reason for
    /// this is that Holochain is optimized for these three query types, while
    /// kitsune has a more general interface.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_query_agents(
        &mut self,
        input: kitsune_p2p::event::QueryAgentsEvt,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<Vec<AgentInfoSigned>> {
        let kitsune_p2p::event::QueryAgentsEvt {
            space,
            agents,
            window,
            arq_set,
            near_basis,
            limit,
        } = input;

        let h_space = DnaHash::from_kitsune(&space);
        let evt_sender = self.evt_sender.clone();

        Ok(async move {
            let agents = match (agents, window, arq_set, near_basis, limit) {
                // If only basis and limit are set, this is a "near basis" query
                (None, None, None, Some(basis), Some(limit)) => {
                    evt_sender
                        .query_agent_info_signed_near_basis(h_space, space, basis.as_u32(), limit)
                        .await?
                }

                // If arc_set is set, this is a "gossip agents" query
                (agents, window, Some(arq_set), None, None) => {
                    let window = window.unwrap_or_else(full_time_window);
                    let h_agents =
                        agents.map(|agents| agents.iter().map(AgentPubKey::from_kitsune).collect());
                    let since_ms = window.start.as_millis().max(0) as u64;
                    let until_ms = window.end.as_millis().max(0) as u64;
                    evt_sender
                        .query_gossip_agents(
                            h_space,
                            h_agents,
                            space,
                            since_ms,
                            until_ms,
                            arq_set.to_dht_arc_set_std().into(),
                        )
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

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_query_peer_density(
        &mut self,
        space: Arc<kitsune_p2p::KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> kitsune_p2p::event::KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView> {
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

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
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

macro_rules! timing_trace_out {
    ($code:expr, $($rest:tt)*) => {{
        let __start = std::time::Instant::now();
        let __out = $code;
        Ok(async move {
            let __out = __out.await;
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
        }
        .boxed()
        .into())
    }};
}

impl ghost_actor::GhostHandler<HolochainP2p> for HolochainP2pActor {}

impl HolochainP2pHandler for HolochainP2pActor {
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_join(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
        initial_arq: Option<Arq>,
    ) -> HolochainP2pHandlerResult<()> {
        let space = dna_hash.into_kitsune();
        let agent = agent_pub_key.into_kitsune();

        let kitsune_p2p = self.kitsune_p2p.clone();
        Ok(async move {
            Ok(kitsune_p2p
                .join(space, agent, maybe_agent_info, initial_arq)
                .await?)
        }
        .boxed()
        .into())
    }

    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
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

    /// Dispatch an outgoing remote call.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_call_remote(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        let space = dna_hash.into_kitsune();
        let to_agent_kitsune = to_agent.clone().into_kitsune();

        let byte_count = zome_call_params_serialized.0.len();

        let req =
            crate::wire::WireMessage::call_remote(to_agent, zome_call_params_serialized, signature)
                .encode()?;

        let kitsune_p2p = self.kitsune_p2p.clone();
        timing_trace_out!(
            async move {
                let result: Vec<u8> = kitsune_p2p
                    .rpc_single(space, to_agent_kitsune, req, None)
                    .await?;
                Ok(UnsafeBytes::from(result).into())
            },
            byte_count,
            a = "send_call_remote"
        )
    }

    /// Dispatch an outgoing signal.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self), level = "trace")
    )]
    fn handle_send_remote_signal(
        &mut self,
        dna_hash: DnaHash,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> HolochainP2pHandlerResult<()> {
        let byte_count = to_agent_list
            .first()
            .map(|to_agent| to_agent.1 .0.len())
            .unwrap_or_else(|| 0);
        let space = dna_hash.into_kitsune();

        let req = crate::wire::WireMessage::call_remote_multi(to_agent_list.clone()).encode()?;

        let timeout = self.config.tuning_params.implicit_timeout();

        let to_agents = to_agent_list
            .iter()
            .map(|(agent, _zome_call_payload, _signature)| agent.clone().into_kitsune())
            .collect();
        let kitsune_p2p = self.kitsune_p2p.clone();
        timing_trace_out!(
            async move {
                kitsune_p2p
                    .targeted_broadcast(space, to_agents, timeout, req, true)
                    .await?;
                Ok(())
            },
            byte_count,
            a = "send_remote_signal"
        )
    }

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
}
