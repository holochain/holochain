use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::context_log::CtxError;
use crate::*;
use aitia::cause::CauseResult;
use aitia::graph::CauseError;
use aitia::logging::FactLogTraits;
use aitia::{Cause, FactTraits};
use holochain_types::prelude::*;

/// A DhtOpLite along with its corresponding DhtOpHash
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    derive_more::Constructor,
    derive_more::Deref,
    derive_more::Into,
)]
pub struct OpInfo {
    #[deref]
    pub(crate) op: DhtOpLite,
    pub(crate) hash: DhtOpHash,
    pub(crate) dep: SysValDep,
}

impl OpInfo {
    /// Accessor
    pub fn as_hash(&self) -> &DhtOpHash {
        &self.hash
    }
}

pub type OpRef = DhtOpHash;

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Into,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct OpAction(pub ActionHash, pub DhtOpType);

impl From<OpLite> for OpAction {
    fn from(op: OpLite) -> Self {
        Self(op.action_hash().clone(), op.get_type())
    }
}

impl OpAction {
    pub fn action_hash(&self) -> &ActionHash {
        &self.0
    }

    pub fn op_type(&self) -> &DhtOpType {
        &self.1
    }
}

pub type SleuthId = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Step {
    /// The node has published this at least once, to somebody
    Published {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has gossiped this at least once, to somebody
    Gossiped {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has integrated an op authored by someone else
    Integrated {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has app validated an op authored by someone else
    AppValidated {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has sys validated an op authored by someone else
    SysValidated {
        by: SleuthId,
        op: OpRef,
    },

    /// TODO: handle a missing app validation dep
    MissingAppValDep {
        by: SleuthId,
        op: OpRef,
        deps: Vec<AnyDhtHash>,
    },
    /// The node has fetched an op after hearing about the hash via publish or gossip
    Fetched {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has received an op hash via publish or gossip
    ReceivedHash {
        by: SleuthId,
        op: OpRef,
    },
    /// The node has authored this op, including validation and integration
    Authored {
        by: AgentPubKey,
        op: OpInfo,
    },
    /// An agent has joined the network
    AgentJoined {
        node: SleuthId,
        agent: AgentPubKey,
    },
    // XXX: this is a replacement for a proper AgentLeave. This just lets us act as if every
    // agent in the SweetConductor has left
    SweetConductorShutdown {
        node: SleuthId,
    },
}

impl aitia::logging::FactLogJson for Step {}

impl aitia::Fact for Step {
    type Context = Context;

    fn explain(&self, ctx: &Self::Context) -> String {
        match self {
            Step::Published { by, op } => format!("[{}] Published: {:?}", by, op),
            Step::Gossiped { by, op } => format!("[{}] Gossiped: {:?}", by, op),
            Step::Integrated { by, op } => {
                format!("[{}] Integrated: {:?}", by, op)
            }
            Step::AppValidated { by, op } => {
                format!("[{}] AppValidated: {:?}", by, op)
            }
            Step::SysValidated { by, op } => {
                format!("[{}] SysValidated: {:?}", by, op)
            }
            Step::MissingAppValDep { by, op, deps } => {
                format!("[{}] PendingAppValidation: {:?} deps: {:#?}", by, op, deps)
            }
            Step::Fetched { by, op } => format!("[{}] Fetched: {:?}", by, op),
            Step::ReceivedHash { by, op } => format!("[{}] ReceivedHash: {:?}", by, op),
            Step::Authored { by, op } => {
                let node = ctx.agent_node(&by).expect("I got lazy");
                let op_hash = op.as_hash();
                format!("[{}] Authored: {:?}", node, op_hash)
            }
            Step::AgentJoined { node, agent } => {
                format!("[{}] AgentJoined: {:?}", node, agent)
            }
            Step::SweetConductorShutdown { node } => {
                format!("[{}] SweetConductorShutdown", node)
            }
        }
    }

    fn cause(&self, ctx: &Self::Context) -> CauseResult<Self> {
        use Step::*;

        let mapper = |e: CtxError| CauseError {
            info: e.into(),
            fact: Some(self.clone()),
        };

        Ok(match self.clone() {
            Published { by, op } => Some(Self::authority(ctx, by, op)?),
            Gossiped { by, op } => Some(Self::authority(ctx, by, op)?),
            Integrated { by, op } => Some(
                AppValidated {
                    by: by.clone(),
                    op: op.clone(),
                }
                .into(),
            ),
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),
            MissingAppValDep { by, op, deps: _ } => Some(Cause::from(SysValidated { by, op })),
            SysValidated { by, op } => {
                let op_info = ctx.op_info(&op).map_err(mapper)?;
                let dep = ctx.sysval_op_dep(&op).map_err(mapper)?;

                let fetched = Fetched {
                    by: by.clone(),
                    op: op.clone(),
                }
                .into();

                if let Some(dep) = dep {
                    let integrated = Cause::from(Integrated { by, op });
                    // TODO: eventually we don't want to just use anything we fetched, right?
                    Some(Cause::Every("Exists".into(), vec![fetched, integrated]))
                } else {
                    Some(fetched)
                }
            }
            Fetched { by, op } => Some(ReceivedHash { by, op }.into()),
            ReceivedHash { by, op } => {
                let mut others: Vec<_> = ctx
                    .map_node_to_agents
                    .keys()
                    .filter(|i| **i != by)
                    .cloned()
                    .map(|i| {
                        // TODO: this should be Published | Gossiped, but we
                        // don't have a good rule for Gossiped yet
                        Integrated {
                            by: i,
                            op: op.clone(),
                        }
                        .into()
                    })
                    .collect();
                Some(Cause::Any("Peer authorities".into(), others))
            }
            Authored { by, op } => {
                let node = ctx.agent_node(&by).map_err(mapper)?.clone();
                Some(Cause::from(AgentJoined { node, agent: by }))
            }
            AgentJoined { node, agent } => None,
            SweetConductorShutdown { node } => None,
        })
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}

impl Step {
    /// The cause which is satisfied by either Integrating this op,
    /// or having authored this op by any of the local agents
    pub fn authority(
        ctx: &Context,
        by: SleuthId,
        op: OpRef,
    ) -> Result<Cause<Self>, CauseError<Self>> {
        let integrated = Self::Integrated {
            by: by.clone(),
            op: op.clone(),
        }
        .into();
        let mut any = vec![integrated];

        let mapper = |e: CtxError| CauseError {
            info: e.into(),
            fact: None,
        };

        let op_info = ctx.op_info(&op).map_err(mapper)?;
        let authors = ctx
            .node_agents(&by)
            .map_err(mapper)?
            .into_iter()
            .cloned()
            .map(|agent| Self::Authored {
                by: agent,
                op: op_info.clone(),
            })
            .map(Cause::from);

        any.extend(authors);
        Ok(Cause::Any("Authority".into(), any))
    }
}
