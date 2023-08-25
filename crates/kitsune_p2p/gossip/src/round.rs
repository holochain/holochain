use kitsune_p2p_dht::region_set::RegionSetLtcs;
use kitsune_p2p_types::{
    dht_arc::{DhtArcRange, DhtArcSet},
    KAgent, KAgentInfo,
};

use crate::{
    bloom::{BloomFilter, MetaOpKey},
    codec::{self as msgs, GossipMsg},
};

#[derive(Debug)]
pub struct GossipRound {
    params: GossipRoundParams,
    /// The state of this Round
    stage: GossipRoundStage,
}

impl GossipRound {
    pub fn new(params: GossipRoundParams) -> (Self, Fx) {
        let (stage, fx) = GossipRoundStage::new(params.plan, params.initiator);
        (Self { params, stage }, fx)
    }
}

#[derive(Debug, derive_more::Constructor)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct GossipRoundParams {
    /// The agreed-upon plan during handshake
    plan: GossipPlan,
    /// True if I initiated, false if I accepted an Initiate
    initiator: bool,
}

#[derive(Clone, Debug)]
pub struct GossipRoundState {
    stage: GossipRoundStage,
    common_arc_set: DhtArcSet,
    local_agents: Vec<KAgentInfo>,
}

#[derive(Clone, Debug)]
pub enum GossipRoundStage {
    /// Initiate has been sent, awaiting the Accept.
    /// Includes an optional slot to store an out-of-order message.
    AwaitingAccept(Option<GossipMsg>),
    /// Sent our agent bloom filter, expecting to receive Agent blooms
    ExchangingAgentBlooms,
    /// Sent our agent data, expecting to receive Agent data
    ExchangingAgentData,
    /// We are using bloom filters to communicate about ops, we sent ours
    /// and are expecting the other party's.
    ExchangingOpBlooms,
    /// We are using spacetime regions to communicate about ops, we sent ours
    /// and are expecting the other party's.
    ExchangingOpRegions {
        our_regions: RegionSetLtcs,
        their_regions: Option<RegionSetLtcs>,
    },
    /// Sending and receiving op batches
    CollectingOpData,
    /// All done
    Finished,
}
type Stage = GossipRoundStage;

impl GossipRoundStage {
    pub fn new(plan: GossipPlan, initiator: bool) -> (Self, Fx) {
        if initiator {
            (Self::AwaitingAccept(None), FxSend::Initiate(plan).into())
        } else if plan.exchange_agents {
            (
                Self::ExchangingAgentBlooms,
                FxSend::Accept(Box::new(FxSend::SendAgentDiff)).into(),
            )
        } else {
            (
                Self::ExchangingOpBlooms,
                FxSend::Accept(Box::new(FxSend::SendOpBloom)).into(),
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct GossipPlan {
    pub exchange_agents: bool,
    pub diff_type: GossipDiffType,
}

impl GossipPlan {
    pub fn initial_trigger(&self) -> FxSend {
        if self.exchange_agents {
            FxSend::SendAgentDiff
        } else {
            FxSend::SendOpBloom
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub enum GossipDiffType {
    Bloom,
    Regions,
}

#[derive(Debug, derive_more::From)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub enum Ax {
    Initiate(AxInitiate),
    Accept(AxAccept),
    AgentDiff(AxAgentDiff),
    AgentData(AxAgentData),
    OpBloom(AxOpBloom),
    OpRegions(AxOpRegions),
    OpData(AxOpData),
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxInitiate {
    pub msg: msgs::Initiate,
    pub local_agents: Vec<KAgent>,
    pub local_arcset: DhtArcSet,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxAccept {
    pub msg: msgs::Accept,
    pub local_agents: Vec<KAgent>,
    pub local_arcset: Vec<DhtArcRange>,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxAgentDiff {
    pub msg: msgs::AgentDiff,
    pub local_agents: Vec<KAgentInfo>,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxAgentData {
    pub msg: msgs::AgentData,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxOpBloom {
    pub msg: msgs::OpBloom,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxOpRegions {
    pub msg: msgs::OpRegions,
}

#[derive(Debug)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct AxOpData {
    pub msg: msgs::OpData,
}

#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub enum Fx {
    Metric(FxMetric),
    Put(FxPut),
    Send(FxSend),
    Msg(GossipMsg),
}

#[derive(Debug, PartialEq, Eq)]
pub enum FxMetric {
    Latency(Vec<KAgent>, Microseconds),
}

type Microseconds = u128;

#[derive(Debug, PartialEq, Eq)]
pub enum FxPut {
    PutAgentInfo(Vec<KAgentInfo>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum FxSend {
    Initiate(GossipPlan),
    Accept(Box<FxSend>),
    SendAgentDiff,
    SendAgentData(Vec<KAgentInfo>),
    SendOpBloom,
    SendOpData,
    UnexpectedMessage,
}

impl stef::ParamState for GossipRound {
    type Action = Ax;
    type Effect = Vec<Fx>;
    type State = GossipRoundStage;
    type Params = GossipRoundParams;

    fn initial(params: Self::Params) -> Self {
        let (stage, _) = GossipRoundStage::new(params.plan, params.initiator);
        Self { stage, params }
    }

    fn partition(&mut self) -> (&mut Self::State, &Self::Params) {
        (&mut self.stage, &self.params)
    }

    fn update(state: &mut Self::State, params: &Self::Params, ax: Self::Action) -> Self::Effect {
        stef::update_replace(state, |state| match (state, ax) {
            (Stage::AwaitingAccept(queued_todo), Ax::Accept(a)) => {
                state.incoming_accept(&params.plan, a)
            }
            (Stage::ExchangingAgentBlooms, Ax::AgentDiff(a)) => state.incoming_agent_bloom(a),
            (Stage::ExchangingAgentData, Ax::AgentData(a)) => state.incoming_agent_data(a),
            (Stage::ExchangingOpBlooms, Ax::OpBloom(a)) => state.incoming_op_bloom(a),
            (Stage::ExchangingOpRegions { .. }, Ax::OpRegions(a)) => state.incoming_op_regions(a),

            (stage @ Stage::CollectingOpData, Ax::OpData(a)) => {
                let finished = todo!();
                if finished {
                    (Stage::Finished, vec![])
                } else {
                    (*stage, todo!())
                }
            }
            (Stage::Finished, msg) => {
                tracing::debug!("message received after round finished: {:?}", msg);
                (Stage::Finished, vec![])
            }
            (GossipRoundStage::AwaitingAccept(_), Ax::Initiate(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::Accept(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::AgentDiff(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::AgentData(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::OpBloom(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::OpRegions(_)) => todo!(),
            (GossipRoundStage::AwaitingAccept(_), Ax::OpData(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::Initiate(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::Accept(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::AgentDiff(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::AgentData(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::OpBloom(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::OpRegions(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentBlooms, Ax::OpData(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::Initiate(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::Accept(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::AgentDiff(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::AgentData(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::OpBloom(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::OpRegions(_)) => todo!(),
            (GossipRoundStage::ExchangingAgentData, Ax::OpData(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::Initiate(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::Accept(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::AgentDiff(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::AgentData(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::OpBloom(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::OpRegions(_)) => todo!(),
            (GossipRoundStage::ExchangingOpBlooms, Ax::OpData(_)) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::Initiate(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::Accept(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::AgentDiff(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::AgentData(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::OpBloom(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::OpRegions(_),
            ) => todo!(),
            (
                GossipRoundStage::ExchangingOpRegions {
                    our_regions,
                    their_regions,
                },
                Ax::OpData(_),
            ) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::Initiate(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::Accept(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::AgentDiff(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::AgentData(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::OpBloom(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::OpRegions(_)) => todo!(),
            (GossipRoundStage::CollectingOpData, Ax::OpData(_)) => todo!(),
        })
    }
}

impl GossipRoundStage {
    /// ### Effects
    /// - Send agent diff
    /// - Record latency metric
    fn incoming_accept(&self, plan: &GossipPlan, ax: AxAccept) -> (GossipRoundStage, Vec<Fx>) {
        if ax.local_agents.is_empty() {
            return (self.clone(), vec![GossipMsg::no_agents().into()]);
        }

        if *plan != ax.msg.plan {
            todo!("error")
        }

        let common_arcset = todo!();
        let local_agents_within_arcset = todo!();

        let (stage, msg) = if plan.exchange_agents {
            let agent_bloom = generate_agent_bloom(local_agents_within_arcset);
            (
                Stage::ExchangingAgentBlooms,
                GossipMsg::agent_diff(agent_bloom),
            )
        } else {
            match plan.diff_type {
                GossipDiffType::Bloom => (
                    Stage::ExchangingAgentBlooms,
                    GossipMsg::op_bloom(todo!(), todo!()),
                ),
                GossipDiffType::Regions => {
                    (Stage::ExchangingAgentBlooms, GossipMsg::op_regions(todo!()))
                }
            }
        };
        let fx = vec![
            Fx::Msg(msg),
            FxMetric::Latency(ax.local_agents.clone(), todo!("latency")).into(),
            todo!("other metrics"),
        ];
        (stage, fx)
    }

    /// ### Effects
    /// - Send agent data
    fn incoming_agent_bloom(&self, ax: AxAgentDiff) -> (GossipRoundStage, Vec<Fx>) {
        let bloom = ax.msg.bloom_filter;
        let missing: Vec<_> = ax
            .local_agents
            .into_iter()
            .filter(|info| {
                // Check them against the bloom
                !bloom.check(&MetaOpKey::Agent(info.agent.clone(), info.signed_at_ms))
            })
            .collect();

        let fx = if !missing.is_empty() {
            vec![FxSend::SendAgentData(missing).into()]
        } else {
            // It's ok if we don't respond to agent blooms because
            // rounds are ended by ops not agents.
            vec![]
        };

        (Stage::ExchangingAgentData, fx)
    }

    /// ### Effects
    /// - Store agent info
    fn incoming_agent_data(&self, ax: AxAgentData) -> (GossipRoundStage, Vec<Fx>) {
        let agents = ax.msg.agents;
        let fx = vec![
            FxPut::PutAgentInfo(agents).into(),
            FxSend::SendOpBloom.into(),
        ];

        (Stage::ExchangingOpBlooms, fx)
    }

    /// ### Effects
    fn incoming_op_bloom(&self, ax: AxOpBloom) -> (GossipRoundStage, Vec<Fx>) {
        (Stage::CollectingOpData, vec![FxSend::SendOpData.into()])
    }

    /// ### Effects
    fn incoming_op_regions(&self, ax: AxOpRegions) -> (GossipRoundStage, Vec<Fx>) {
        todo!()
    }
}

fn generate_agent_bloom(agents: Vec<KAgent>) -> BloomFilter {
    todo!()
}

fn unexpected() -> (GossipRoundStage, Vec<Fx>) {
    (
        GossipRoundStage::Finished,
        vec![FxSend::UnexpectedMessage.into()],
    )
}
