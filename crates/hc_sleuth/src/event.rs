use std::fmt::Debug;
use std::hash::Hash;

use crate::context_log::CtxError;
use crate::*;
use aitia::Dep;
use aitia::DepError;
use aitia::DepResult;
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
    pub(crate) dep: SysValDeps,
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
pub struct ChainOpAction(pub ActionHash, pub ChainOpType);

impl From<ChainOpLite> for ChainOpAction {
    fn from(op: ChainOpLite) -> Self {
        Self(op.action_hash().clone(), op.get_type())
    }
}

impl ChainOpAction {
    pub fn action_hash(&self) -> &ActionHash {
        &self.0
    }

    pub fn op_type(&self) -> &ChainOpType {
        &self.1
    }
}

pub type SleuthId = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub fact: Fact,
    pub timestamp: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Fact {
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

impl aitia::logging::FactLogMsgpackB64 for Event {}

impl aitia::Fact for Fact {
    type Context = Context;

    fn explain(&self, ctx: &Self::Context) -> String {
        match self {
            Fact::Integrated { by, op } => {
                format!("[{by}] {op} Integrated")
            }
            Fact::AppValidated { by, op } => {
                format!("[{by}] {op} AppValidated")
            }
            Fact::SysValidated { by, op } => {
                format!("[{by}] {op} SysValidated")
            }
            Fact::MissingAppValDep { by, op, deps } => {
                format!("[{by}] {op} PendingAppValidation deps: {deps:#?}")
            }
            Fact::Fetched { by, op } => format!("[{by}] {op} Fetched"),
            Fact::SentHash { by, op, method } => format!("[{by}] {op} SentHash({method})"),
            Fact::ReceivedHash { by, op, method } => {
                format!("[{by}] {op} ReceivedHash({method})")
            }
            Fact::Authored { by, op } => {
                let node = ctx
                    .agent_node(by)
                    .cloned()
                    .unwrap_or("???unknown???".to_string());
                let op_hash = op.as_hash();
                format!("[{node}] {op_hash} Authored")
            }
            Fact::AgentJoined { node, agent } => {
                format!("[{node}] {agent} AgentJoined")
            }
            Fact::SweetConductorShutdown { node } => {
                format!("[{node}] SweetConductorShutdown")
            }
        }
    }

    fn dep(&self, ctx: &Self::Context) -> DepResult<Self> {
        use Fact::*;

        let mapper = |e: CtxError| DepError {
            info: e,
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
            MissingAppValDep {
                by: _,
                op: _,
                deps: _,
            } => todo!(),

            // Ops can only be sys validated after being fetched from an authority, and after
            // its dependency has been integrated
            SysValidated { by, op } => {
                let dep = ctx.sysval_op_deps(&op).map_err(mapper)?;

                let fetched = Fetched {
                    by: by.clone(),
                    op: op.clone(),
                }
                .into();

                // TODO: this must be generalized to support multiple dependencies.
                //       we're only using the first here.
                if let Some(dep) = dep.first() {
                    let integrated = Dep::from(Integrated {
                        by,
                        op: dep.hash.clone(),
                    });
                    // TODO: eventually we don't want to just use anything we fetched, right?
                    // TODO: currently we don't actually need to integrate the dep, it can just exist in the cache
                    Some(Dep::every_named("Exists", vec![fetched, integrated]))
                } else {
                    Some(fetched)
                }
            }

            // An op can be fetched only if its hash is in the fetch pool, which happens
            // whenever the op is received by any method
            Fetched { by, op } => Some(Dep::any_named(
                "ReceivedHash",
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
                let others: Vec<_> = ctx
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
                Some(Dep::any_named("Received hash from authority", others))
            }

            // An agent can author an op at any time, but must have joined the network first
            Authored { by, op: _ } => {
                let node = ctx.agent_node(&by).map_err(mapper)?.clone();
                Some(Dep::from(AgentJoined { node, agent: by }))
            }

            // An agent can join at any time
            AgentJoined { .. } => None,

            // "Special" cause
            SweetConductorShutdown { .. } => None,
        })
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}

impl Fact {
    pub fn now(self) -> Event {
        Event {
            fact: self,
            timestamp: Timestamp::now(),
        }
    }

    /// The cause which is satisfied by either Integrating this op,
    /// or having authored this op by any of the local agents
    #[allow(clippy::result_large_err)]
    pub fn authority(ctx: &Context, by: SleuthId, op: OpRef) -> Result<Dep<Self>, DepError<Self>> {
        let integrated = Self::Integrated {
            by: by.clone(),
            op: op.clone(),
        }
        .into();
        let mut any = vec![integrated];

        let mapper = |e: CtxError| DepError {
            info: e,
            fact: None,
        };

        let op_info = ctx.op_info(&op).map_err(mapper)?;
        let authors = ctx
            .node_agents(&by)
            .map_err(mapper)?
            .iter()
            .cloned()
            .map(|agent| Self::Authored {
                by: agent,
                op: op_info.clone(),
            })
            .map(Dep::from);

        any.extend(authors);
        Ok(Dep::any_named("Authority", any))
    }

    pub fn op(&self) -> Option<&OpRef> {
        use Fact::*;
        match self {
            Integrated { op, .. }
            | AppValidated { op, .. }
            | SysValidated { op, .. }
            | MissingAppValDep { op, .. }
            | Fetched { op, .. }
            | SentHash { op, .. }
            | ReceivedHash { op, .. } => Some(op),

            Authored { op, .. } => Some(op.as_hash()),

            AgentJoined { .. } | SweetConductorShutdown { .. } => None,
        }
    }
}
