use std::collections::HashMap;
use std::sync::Arc;

use crate::event::*;
use crate::types::event::{KitsuneP2pEvent, KitsuneP2pEventHandler, KitsuneP2pEventHandlerResult};
use crate::types::gossip::GossipModule;
use crate::types::wire;
use kitsune_p2p_types::agent_info::{AgentInfoInner, AgentInfoSigned};
use kitsune_p2p_types::bin_types::*;
use kitsune_p2p_types::dht_arc::loc8::Loc8;
use kitsune_p2p_types::dht_arc::{ArcInterval, DhtArc, DhtLocation};
use kitsune_p2p_types::tx2::tx2_api::Tx2EpHnd;
use kitsune_p2p_types::tx2::tx2_utils::Share;
use kitsune_p2p_types::*;

type KSpace = Arc<KitsuneSpace>;
type KAgent = Arc<KitsuneAgent>;
type KOpHash = Arc<KitsuneOpHash>;

#[derive(Clone)]
pub struct SwitchboardNode {
    space: KSpace,
    gossip: GossipModule,
    ep_hnd: Tx2EpHnd<wire::Wire>,
    state: Share<SwitchboardNodeState>,
}

impl std::hash::Hash for SwitchboardNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ep_hnd.hash(state)
    }
}

#[derive(Clone)]
pub struct SwitchboardEventHandler {
    space: KSpace,
    state: Share<SwitchboardNodeState>,
}

impl SwitchboardEventHandler {
    pub fn new(space: KSpace) -> Self {
        Self {
            space,
            state: Share::new(Default::default()),
        }
    }

    fn assert_space(&self, space: KSpace) -> () {
        assert_eq!(self.space, space, "Got query for unexpected space");
    }
}

#[derive(Default)]
pub struct SwitchboardNodeState {
    agents: HashMap<KAgent, AgentInfoSigned>,
    ops: HashMap<KOpHash, Vec<u8>>,
}

impl SwitchboardNode {
    pub fn new(
        handler: SwitchboardEventHandler,
        gossip: GossipModule,
        ep_hnd: Tx2EpHnd<wire::Wire>,
    ) -> Self {
        Self {
            space: handler.space,
            gossip,
            ep_hnd,
            state: handler.state,
        }
    }

    pub fn add_agents<A>(&self, agents: A)
    where
        A: IntoIterator<Item = (Loc8, ArcInterval<Loc8>)>,
    {
        // TODO: make the switchboard add this agent info to all other nodes too
        let space = self.space.clone();
        let new_agents: Vec<KAgent> = self
            .state
            .share_mut(|state, _| {
                let info = agents
                    .into_iter()
                    .map(|(agent_loc, arc): (Loc8, ArcInterval<Loc8>)| {
                        let agent_loc: DhtLocation = agent_loc.into();
                        let agent = Arc::new(KitsuneAgent::new(agent_loc.to_bytes_36()));
                        (
                            agent.clone(),
                            fake_agent_info(space.clone(), agent, arc.to_dht_location()),
                        )
                    })
                    .collect::<Vec<_>>();
                let new_agents = info.iter().map(|(agent, _)| agent).cloned().collect();
                state.agents.extend(info);
                Ok(new_agents)
            })
            .unwrap();
        for agent in new_agents {
            self.gossip.local_agent_join(agent);
        }
    }

    pub fn add_ops<O>(&self, ops: O)
    where
        O: IntoIterator<Item = Loc8>,
    {
        self.state
            .share_mut(|state, _| {
                state.ops.extend(ops.into_iter().map(|op_loc: Loc8| {
                    let loc: DhtLocation = op_loc.into();
                    let hash = Arc::new(KitsuneOpHash::new(loc.to_bytes_36()));
                    let data = loc.as_u32().to_le_bytes().to_vec();
                    (hash, data)
                }));
                Ok(())
            })
            .unwrap();
        self.gossip.new_integrated_data();
    }

    pub fn get_ops(&self) -> Vec<Loc8> {
        self.state
            .share_ref(|state| {
                let mut ops: Vec<_> = state.ops.keys().map(|hash| hash.get_loc().into()).collect();
                ops.sort();
                Ok(ops)
            })
            .unwrap()
    }
}

fn fake_agent_info(space: KSpace, agent: KAgent, interval: ArcInterval) -> AgentInfoSigned {
    let state = AgentInfoInner {
        space,
        agent,
        storage_arc: DhtArc::from_interval(interval),
        url_list: vec![],
        signed_at_ms: 0,
        expires_at_ms: 0,
        signature: Arc::new(KitsuneSignature(vec![])),
        encoded_bytes: Box::new([]),
    };
    AgentInfoSigned(Arc::new(state))
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for SwitchboardEventHandler {}
impl ghost_actor::GhostControlHandler for SwitchboardEventHandler {}

#[allow(warnings)]
impl KitsuneP2pEventHandler for SwitchboardEventHandler {
    fn handle_put_agent_info_signed(
        &mut self,
        PutAgentInfoSignedEvt { space, peer_data }: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        dbg!("handle_put_agent_info_signed");
        self.assert_space(space);
        self.state.share_mut(|state, _| {
            state
                .agents
                .extend(peer_data.into_iter().map(|d| (d.agent.clone(), d)));
            Ok(())
        })?;
        ok_fut(Ok(()))
    }

    fn handle_get_agent_info_signed(
        &mut self,
        GetAgentInfoSignedEvt { space, agent }: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        dbg!("handle_get_agent_info_signed");
        self.assert_space(space);
        ok_fut(Ok(self
            .state
            .share_mut(|state, _| Ok(state.agents.get(&agent).cloned()))?))
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
        let result = self.state.share_mut(|state, _| {
            Ok(if let Some(agents) = agents {
                state
                    .agents
                    .iter()
                    .filter(|(agent, _)| agents.contains(*agent))
                    .map(|(_, info)| info)
                    .cloned()
                    .collect()
            } else {
                state.agents.iter().map(|(_, info)| info).cloned().collect()
            })
        })?;
        ok_fut(Ok(dbg!(result)))
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
