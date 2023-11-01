use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::*;
use aitia::cause::CauseResult;
use aitia::graph::CauseError;
use aitia::logging::FactLogTraits;
use aitia::{Cause, FactTraits};
use holochain_state::{prelude::*, validation_db::ValidationStage};

// #[derive(Debug, Clone, derive_more::From)]
// pub enum OpRef {
//     OpLite(OpLite),
//     Action(ActionHash, DhtOpType),
// }

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

pub type OpLite = DhtOpLite;
pub type NodeId = String;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Step {
    Authored {
        by: NodeId,
        action: ActionHash,
    },
    Published {
        by: NodeId,
        op: OpAction,
    },
    Integrated {
        by: NodeId,
        op: OpAction,
    },
    AppValidated {
        by: NodeId,
        op: OpAction,
    },
    SysValidated {
        by: NodeId,
        op: OpAction,
    },
    PendingSysValidation {
        by: NodeId,
        op: OpAction,
        dep: Option<AnyDhtHash>,
    },
    PendingAppValidation {
        by: NodeId,
        op: OpAction,
        deps: Vec<AnyDhtHash>,
    },
    Fetched {
        by: NodeId,
        op: OpAction,
    },
    Seen {
        by: NodeId,
        op_lite: OpLite,
    },
    // GossipReceived {},
    // PublishReceived {},
}

impl aitia::logging::FactLogJson for Step {}

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Authored { by, action } => {
                f.write_fmt(format_args!("[{}] Authored: {}", by, action))
            }
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
            Step::PendingSysValidation { by, op, dep } => f.write_fmt(format_args!(
                "[{}] PendingSysValidation: {:?} dep: {:?}",
                by, op, dep
            )),
            Step::PendingAppValidation { by, op, deps } => f.write_fmt(format_args!(
                "[{}] PendingAppValidation: {:?} deps: {:#?}",
                by, op, deps
            )),
            Step::Fetched { by, op } => f.write_fmt(format_args!("[{}] Fetched: {:?}", by, op)),
            Step::Seen { by, op_lite } => {
                f.write_fmt(format_args!("[{}] Op Seen: {:?}", by, op_lite))
            }
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

        Ok(match self.clone() {
            Authored { by, action } => None,
            Published { by, op } => Some(Integrated { by, op }.into()),
            Integrated { by, op } => Some(AppValidated { by, op }.into()),
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),
            PendingAppValidation { by, op, deps: _ } => Some(Cause::from(SysValidated { by, op })),
            SysValidated { by, op } => {
                let dep = ctx
                    .sysval_op_dep(&op)
                    .map_err(|_| CauseError(self.clone()))?;
                let pending = PendingSysValidation {
                    by: by.clone(),
                    op: op.clone(),
                    dep: dep.clone().map(|d| d.fetch_dependency_hash()),
                }
                .into();

                if let Some(dep) = dep {
                    let integrated = Cause::from(Integrated { by, op });
                    Some(Cause::Every(vec![pending, integrated]))
                } else {
                    Some(pending)
                }
            }
            PendingSysValidation { by, op, dep: _ } => Some(
                Seen {
                    by,
                    op_lite: ctx
                        .action_to_op(&op)
                        .map_err(|_| CauseError(self.clone()))?,
                }
                .into(),
            ),
            Seen { by, op_lite } => {
                let authored = Authored {
                    by: by.clone(),
                    action: op_lite.action_hash().clone(),
                };

                let fetched = Fetched {
                    by: by.clone(),
                    op: op_lite.into(),
                };

                Some(Cause::Any(vec![authored.into(), fetched.into()]))
            }
            Fetched { by, op } => {
                let mut others: Vec<_> = ctx
                    .node_ids()
                    .iter()
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
        })
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}
