use super::*;

type KAgent = Arc<KitsuneAgent>;
type KAgentMap = HashMap<KAgent, Arc<AgentInfoSigned>>;

ghost_actor::ghost_chan! {
    /// controller for test harness agent actor
    pub(crate) chan HarnessAgentControl<KitsuneP2pError> {
        /// dump agent info from peer_store
        fn dump_agent_info() -> Vec<Arc<AgentInfoSigned>>;

        /// inject a bunch of agent info
        fn inject_agent_info(info: KAgentMap) -> ();

        /// inject data to be gradually gossiped
        fn inject_gossip_data(data: String) -> Arc<KitsuneOpHash>;

        /// dump all local gossip data from this agent
        fn dump_local_gossip_data() -> HashMap<Arc<KitsuneOpHash>, String>;

        /// dump all local peer data from this agent
        fn dump_local_peer_data() -> HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>;
    }
}

pub struct HarnessHost;

impl HarnessHost {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl KitsuneHostPanicky for HarnessHost {
    const NAME: &'static str = "HarnessHost";

    fn peer_extrapolated_coverage(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>> {
        box_fut(Ok(vec![]))
    }

    fn query_region_set(
        &self,
        _space: Arc<KitsuneSpace>,
        _dht_arc_set: Arc<DhtArcSet>,
    ) -> KitsuneHostResult<RegionSetLtcs> {
        box_fut(Ok(RegionSetLtcs::empty()))
    }
}

pub(crate) async fn spawn_test_agent(
    harness_chan: HarnessEventChannel,
    config: KitsuneP2pConfig,
) -> Result<
    (
        Arc<KitsuneAgent>,
        ghost_actor::GhostSender<KitsuneP2p>,
        ghost_actor::GhostSender<HarnessAgentControl>,
    ),
    KitsuneP2pError,
> {
    let topology = Topology::standard_epoch();
    let host = HarnessHost::new();
    let (p2p, evt) = spawn_kitsune_p2p(
        config,
        kitsune_p2p_types::tls::TlsConfig::new_ephemeral()
            .await
            .unwrap(),
        host,
    )
    .await?;

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    channel_factory.attach_receiver(evt).await?;

    let control = channel_factory
        .create_channel::<HarnessAgentControl>()
        .await?;

    let harness = AgentHarness::new(harness_chan, topology).await?;
    let agent = harness.agent.clone();
    tokio::task::spawn(builder.spawn(harness));

    Ok((agent, p2p, control))
}

use kitsune_p2p_timestamp::Timestamp;
use kitsune_p2p_types::box_fut;
use kitsune_p2p_types::dependencies::lair_keystore_api_0_0;
use kitsune_p2p_types::dht::prelude::RegionSetLtcs;
use kitsune_p2p_types::dht::spacetime::Topology;
use kitsune_p2p_types::dht::PeerStrat;
use kitsune_p2p_types::dht_arc::DhtArcSet;
use lair_keystore_api_0_0::entry::EntrySignEd25519;
use lair_keystore_api_0_0::internal::sign_ed25519::*;

struct AgentHarness {
    agent: Arc<KitsuneAgent>,
    priv_key: SignEd25519PrivKey,
    harness_chan: HarnessEventChannel,
    agent_store: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
    gossip_store: HashMap<Arc<KitsuneOpHash>, String>,
    topology: Topology,
}

impl AgentHarness {
    pub async fn new(
        harness_chan: HarnessEventChannel,
        topology: Topology,
    ) -> Result<Self, KitsuneP2pError> {
        let EntrySignEd25519 { priv_key, pub_key } = sign_ed25519_keypair_new_from_entropy()
            .await
            .map_err(KitsuneP2pError::other)?;
        let pub_key = (**pub_key).clone();
        let agent: Arc<KitsuneAgent> = Arc::new(KitsuneAgent::new(pub_key));
        Ok(Self {
            agent,
            priv_key,
            harness_chan,
            agent_store: HashMap::new(),
            gossip_store: HashMap::new(),
            topology,
        })
    }
}

impl ghost_actor::GhostControlHandler for AgentHarness {}

impl ghost_actor::GhostHandler<HarnessAgentControl> for AgentHarness {}

impl HarnessAgentControlHandler for AgentHarness {
    fn handle_dump_agent_info(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<Vec<Arc<AgentInfoSigned>>> {
        let all = self.agent_store.values().cloned().collect();
        Ok(async move { Ok(all) }.boxed().into())
    }

    fn handle_inject_agent_info(
        &mut self,
        info: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
    ) -> HarnessAgentControlHandlerResult<()> {
        self.agent_store.extend(info);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_inject_gossip_data(
        &mut self,
        data: String,
    ) -> HarnessAgentControlHandlerResult<Arc<KitsuneOpHash>> {
        let op_hash: Arc<KitsuneOpHash> = hash_op_data(data.as_bytes());
        self.gossip_store.insert(op_hash.clone(), data);
        Ok(async move { Ok(op_hash) }.boxed().into())
    }

    fn handle_dump_local_gossip_data(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<HashMap<Arc<KitsuneOpHash>, String>> {
        let out = self.gossip_store.clone();
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_dump_local_peer_data(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>> {
        let out = self.agent_store.clone();
        Ok(async move { Ok(out) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for AgentHarness {}

impl KitsuneP2pEventHandler for AgentHarness {
    fn handle_put_agent_info_signed(
        &mut self,
        input: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        for info in input.peer_data {
            let info = Arc::new(info);
            self.agent_store.insert(info.agent.clone(), info.clone());
            self.harness_chan.publish(HarnessEventType::StoreAgentInfo {
                agent: (&info.agent).into(),
                agent_info: info,
            });
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_query_agents(
        &mut self,
        QueryAgentsEvt {
            space: _,
            agents,
            window,
            arc_set,
            near_basis: _,
            limit,
        }: QueryAgentsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        let arc_set = arc_set.unwrap_or_else(|| Arc::new(DhtArcSet::Full));
        let window = window.unwrap_or_else(full_time_window);
        // TODO - sort by near_basis if set
        let out = self
            .agent_store
            .iter()
            .filter(|(a, _)| {
                agents
                    .as_ref()
                    .map(|agents| agents.contains(*a))
                    .unwrap_or(true)
            })
            .filter(|(_, i)| arc_set.contains(i.agent.get_loc()))
            .filter(|(_, i)| window.contains(&Timestamp::from_micros(i.signed_at_ms as i64 * 1000)))
            .take(limit.unwrap_or(u32::MAX) as usize)
            .map(|(_, i)| (**i).clone())
            .collect();
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_query_peer_density(
        &mut self,
        _space: Arc<KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht::PeerView> {
        let strat = PeerStrat::default();
        let arcs: Vec<_> = self.agent_store.values().map(|v| v.storage_arc).collect();

        // contains is already checked in the iterator
        let view = strat.view(self.topology.clone(), dht_arc, arcs.as_slice());

        Ok(async move { Ok(view) }.boxed().into())
    }

    fn handle_call(
        &mut self,
        space: Arc<super::KitsuneSpace>,
        to_agent: Arc<super::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        let data = String::from_utf8_lossy(&payload);
        self.harness_chan.publish(HarnessEventType::Call {
            space: space.into(),
            to_agent: to_agent.into(),
            payload: data.to_string(),
        });
        let data = format!("echo: {}", data);
        let data = data.into_bytes();
        Ok(async move { Ok(data) }.boxed().into())
    }

    fn handle_notify(
        &mut self,
        space: Arc<super::KitsuneSpace>,
        to_agent: Arc<super::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        let data = String::from_utf8_lossy(&payload);
        self.harness_chan.publish(HarnessEventType::Notify {
            space: space.into(),
            to_agent: to_agent.into(),
            payload: data.to_string(),
        });
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_gossip(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        ops: Vec<KOp>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        for op_data in ops {
            // TODO: check that we're handling string data uniformly in both directions
            let op_data = String::from_utf8_lossy(&op_data.0).to_string();
            let op_hash = hash_op_data(op_data.as_bytes());
            self.harness_chan.publish(HarnessEventType::Gossip {
                op_hash: (&op_hash).into(),
                op_data: op_data.clone(),
            });
            self.gossip_store.insert(op_hash, op_data);
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_query_op_hashes(
        &mut self,
        _input: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<super::KitsuneOpHash>>, TimeWindowInclusive)>>
    {
        let hashes: Vec<Arc<super::KitsuneOpHash>> = self.gossip_store.keys().cloned().collect();
        let slug_hashes: Vec<Slug> = hashes.iter().map(|h| h.into()).collect();
        tracing::trace!(?slug_hashes, "FETCH_OP_HASHES");
        Ok(
            async move { Ok(Some((hashes, full_time_window_inclusive()))) }
                .boxed()
                .into(),
        )
    }

    fn handle_fetch_op_data(
        &mut self,
        input: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<super::KitsuneOpHash>, KOp)>> {
        let mut out = Vec::new();
        match input.query {
            FetchOpDataEvtQuery::Hashes(hashes) => {
                for hash in hashes {
                    if let Some(op) = self.gossip_store.get(&hash) {
                        let data = KitsuneOpData::new(op.clone().into_bytes());
                        out.push((hash.clone(), data));
                    }
                }
            }
            FetchOpDataEvtQuery::Regions(_coords) => unimplemented!(),
        }
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        let sig = sign_ed25519(self.priv_key.clone(), input.data);
        Ok(async move {
            let sig = sig.await.map_err(KitsuneP2pError::other)?;
            let sig: Vec<u8> = (**sig).clone();
            Ok(sig.into())
        }
        .boxed()
        .into())
    }
}
