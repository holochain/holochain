use std::collections::HashMap;
use std::sync::Arc;

use crate::event::*;
use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use kitsune_p2p_types::agent_info::{AgentInfoInner, AgentInfoSigned};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArc, DhtLocation};
use kitsune_p2p_types::*;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

// pub struct GossipScenarioNode(RwShare<GossipScenarioNodeState>);

pub struct GossipScenarioNode {
    space: KSpace,
    agents: HashMap<KAgent, AgentInfoSigned>,
    ops: HashMap<KOpHash, Vec<u8>>,
}

impl GossipScenarioNode {
    pub fn new(space: KSpace) -> Self {
        Self {
            space,
            agents: Default::default(),
            ops: Default::default(),
        }
    }

    pub fn add_agents<L, A>(&mut self, agents: A)
    where
        L: Into<DhtLocation>,
        A: Iterator<Item = (L, (L, L))>,
    {
        let space = self.space.clone();
        self.agents
            .extend(agents.map(|(agent_loc, (start, end)): (L, (L, L))| {
                let agent_loc: DhtLocation = agent_loc.into();
                let start: DhtLocation = start.into();
                let end: DhtLocation = end.into();
                let agent = Arc::new(KitsuneAgent::new(agent_loc.to_bytes_36()));
                let interval = ArcInterval::new(start.as_u32(), end.as_u32());
                (
                    agent.clone(),
                    fake_agent_info(space.clone(), agent, interval),
                )
            }));
    }

    pub fn add_ops<L, O>(&mut self, ops: O)
    where
        L: Into<DhtLocation>,
        O: Iterator<Item = L>,
    {
        self.ops.extend(ops.map(|op_loc: L| {
            let loc: DhtLocation = op_loc.into();
            let hash = Arc::new(KitsuneOpHash::new(loc.to_bytes_36()));
            let data = loc.as_u32().to_le_bytes().to_vec();
            (hash, data)
        }));
    }

    fn assert_space(&self, space: KSpace) -> () {
        assert_eq!(self.space, space, "Got query for unexpected space");
    }
}

fn fake_agent_info(space: KSpace, agent: KAgent, interval: ArcInterval) -> AgentInfoSigned {
    let inner = AgentInfoInner {
        space,
        agent,
        storage_arc: DhtArc::from_interval(interval),
        url_list: vec![],
        signed_at_ms: 0,
        expires_at_ms: 0,
        signature: Arc::new(KitsuneSignature(vec![])),
        encoded_bytes: Box::new([]),
    };
    AgentInfoSigned(Arc::new(inner))
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for GossipScenarioNode {}
impl ghost_actor::GhostControlHandler for GossipScenarioNode {}

#[allow(warnings)]
impl KitsuneP2pEventHandler for GossipScenarioNode {
    fn handle_put_agent_info_signed(
        &mut self,
        PutAgentInfoSignedEvt { space, peer_data }: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        self.assert_space(space);
        self.agents
            .extend(peer_data.into_iter().map(|d| (d.agent.clone(), d)));
        ok_fut(Ok(()))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        self.assert_space(space);
        ok_fut(Ok(self.agents.get(&agent).cloned()))
    }

    fn handle_query_agents(
        &mut self,
        QueryAgentsEvt {
            space,
            agents,
            window,
            arc_set,
            near_basis,
            limit,
        }: QueryAgentsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        self.assert_space(space);
        let result = if let Some(agents) = agents {
            self.agents
                .iter()
                .filter(|(agent, _)| agents.contains(*agent))
                .map(|(_, info)| info)
                .cloned()
                .collect()
        } else {
            self.agents.iter().map(|(_, info)| info).cloned().collect()
        };
        ok_fut(Ok(result))
    }

    fn handle_query_peer_density(
        &mut self,
        space: Arc<KitsuneSpace>,
        dht_arc: kitsune_p2p_types::dht_arc::DhtArc,
    ) -> KitsuneP2pEventHandlerResult<kitsune_p2p_types::dht_arc::PeerDensity> {
        self.assert_space(space);
        todo!()
    }

    fn handle_put_metric_datum(&mut self, datum: MetricDatum) -> KitsuneP2pEventHandlerResult<()> {
        todo!()
    }

    fn handle_query_metrics(
        &mut self,
        query: MetricQuery,
    ) -> KitsuneP2pEventHandlerResult<MetricQueryAnswer> {
        todo!()
    }

    fn handle_call(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        self.assert_space(space);
        todo!()
    }

    fn handle_notify(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        from_agent: Arc<KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        self.assert_space(space);
        todo!()
    }

    fn handle_gossip(
        &mut self,
        space: Arc<KitsuneSpace>,
        to_agent: Arc<KitsuneAgent>,
        ops: Vec<(Arc<KitsuneOpHash>, Vec<u8>)>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        self.assert_space(space);
        todo!()
    }

    fn handle_fetch_op_data(
        &mut self,
        input: FetchOpDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<KitsuneOpHash>, Vec<u8>)>> {
        todo!()
    }

    fn handle_query_op_hashes(
        &mut self,
        input: QueryOpHashesEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<(Vec<Arc<KitsuneOpHash>>, TimeWindow)>> {
        todo!()
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        todo!()
    }
}
