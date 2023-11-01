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
    Published {
        by: SleuthId,
        op: OpRef,
    },
    Integrated {
        by: SleuthId,
        op: OpRef,
    },
    AppValidated {
        by: SleuthId,
        op: OpRef,
    },
    SysValidated {
        by: SleuthId,
        op: OpRef,
    },

    PendingAppValidation {
        by: SleuthId,
        op: OpRef,
        deps: Vec<AnyDhtHash>,
    },
    Fetched {
        by: SleuthId,
        op: OpRef,
    },
    Authored {
        by: AgentPubKey,
        op: OpInfo,
    },
    // GossipReceived {},
    // PublishReceived {},
}

impl aitia::logging::FactLogJson for Step {}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Published { by, op } => f.write_fmt(format_args!("[{}] Published: {:?}", by, op)),
            Step::Integrated { by, op } => {
                f.write_fmt(format_args!("[{}] Integrated: {:?}", by, op))
            }
            Step::AppValidated { by, op } => {
                f.write_fmt(format_args!("[{}] AppValidated: {:?}", by, op))
            }
            Step::SysValidated { by, op } => {
                f.write_fmt(format_args!("[{}] SysValidated: {:?}", by, op))
            }

            Step::PendingAppValidation { by, op, deps } => f.write_fmt(format_args!(
                "[{}] PendingAppValidation: {:?} deps: {:#?}",
                by, op, deps
            )),
            Step::Fetched { by, op } => f.write_fmt(format_args!("[{}] Fetched: {:?}", by, op)),
            Step::Authored { by, op } => f.write_fmt(format_args!("[{}] Authored: {:?}", by, op)),
        }
    }
}

impl aitia::Fact for Step {
    type Context = Context;

    fn explain(&self, ctx: &Self::Context) -> String {
        self.to_string()
    }

    fn cause(&self, ctx: &Self::Context) -> CauseResult<Self> {
        use Step::*;

        let mapper = |e: CtxError| CauseError {
            info: e.into(),
            fact: self.clone(),
        };

        Ok(match self.clone() {
            Published { by, op } => Some(Integrated { by, op }.into()),
            Integrated { by, op } => Some(AppValidated { by, op }.into()),
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),
            PendingAppValidation { by, op, deps: _ } => Some(Cause::from(SysValidated { by, op })),
            SysValidated { by, op } => {
                let op_info = ctx.op_info(&op).map_err(mapper)?;
                let dep = ctx.sysval_op_dep(&op).map_err(mapper)?;

                let any = Cause::Any(
                    ctx.node_agents
                        .get(&by)
                        .ok_or_else(|| CauseError::new("node_agents".into(), self.clone()))?
                        .into_iter()
                        .cloned()
                        .map(|agent| Authored {
                            by: agent,
                            op: op_info.clone(),
                        })
                        .chain([Fetched {
                            by: by.clone(),
                            op: op.clone(),
                        }])
                        .map(Cause::from)
                        .collect(),
                );

                if let Some(dep) = dep {
                    let integrated = Cause::from(Integrated { by, op });
                    Some(Cause::Every(vec![any, integrated]))
                } else {
                    Some(any)
                }
            }
            Fetched { by, op } => {
                let mut others: Vec<_> = ctx
                    .node_agents
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
                Some(Cause::Any(others))
            }
            Authored { by, op } => None,
        })
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}
