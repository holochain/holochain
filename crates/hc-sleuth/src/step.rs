use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::context_log::CtxError;
use crate::*;
use aitia::cause::CauseResult;
use aitia::graph::CauseError;
use aitia::logging::FactLogTraits;
use aitia::{Cause, FactTraits};
use holochain_types::prelude::*;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::TransferMethod;

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
    /// The node has published or gossiped this at least once, to somebody
    SentHash {
        by: SleuthId,
        op: OpRef,
        method: TransferMethod,
    },
    /// The node has received an op hash via publish or gossip
    ReceivedHash {
        by: SleuthId,
        op: OpRef,
        method: TransferMethod,
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
            Step::SentHash { by, op, method } => format!("[{by}] SentHash({method}): {op:?}"),
            Step::ReceivedHash { by, op, method } => {
                format!("[{by}] ReceivedHash({method}): {op:?}")
            }
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
            // Op hashes only get gossiped and published by a node after being fully integrated by that node
            // TODO: could add more antecedents
            SentHash { by, op, method: _ } => Some(Self::authority(ctx, by, op)?),

            // Ops get integrated directly after being app validated
            Integrated { by, op } => Some(
                AppValidated {
                    by: by.clone(),
                    op: op.clone(),
                }
                .into(),
            ),

            // Ops get app validated directly after being sys validated
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),

            // TODO
            MissingAppValDep { by, op, deps: _ } => todo!(),

            // Ops can only be sys validated after being fetched from an authority, and after
            // its dependency has been integrated
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

            // An op can be fetched only if its hash is in the fetch pool, which happens
            // whenever the op is received by any method
            Fetched { by, op } => Some(Cause::Any(
                "ReceivedHash".into(),
                [TransferMethod::Publish, TransferMethod::Gossip]
                    .into_iter()
                    .map(|method| {
                        ReceivedHash {
                            by: by.clone(),
                            op: op.clone(),
                            method,
                        }
                        .into()
                    })
                    .collect(),
            )),

            // We can only receive a hash via a given method if some other node has sent it
            // via that method
            ReceivedHash { by, op, method } => {
                let mut others: Vec<_> = ctx
                    .map_node_to_agents
                    .keys()
                    .filter(|i| **i != by)
                    .cloned()
                    .map(|i| {
                        SentHash {
                            by: i,
                            op: op.clone(),
                            method,
                        }
                        .into()
                    })
                    .collect();
                Some(Cause::Any("Peer authorities".into(), others))
            }

            // An agent can author an op at any time, but must have joined the network first
            Authored { by, op } => {
                let node = ctx.agent_node(&by).map_err(mapper)?.clone();
                Some(Cause::from(AgentJoined { node, agent: by }))
            }

            // An agent can join at any time
            AgentJoined { node, agent } => None,

            // "Special" cause
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
