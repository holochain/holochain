use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::*;
use aitia::logging::FactLogTraits;
use aitia::{Cause, FactTraits};
use holochain_state::{prelude::*, validation_db::ValidationStage};

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, derive_more::From, serde::Serialize, serde::Deserialize,
)]
pub struct OpAction(pub ActionHash, pub DhtOpType);

impl From<DhtOp> for OpAction {
    fn from(value: DhtOp) -> Self {
        let t = value.get_type();
        Self(ActionHash::with_data_sync(&value.action()).into(), t)
    }
}

impl From<DhtOpLight> for OpAction {
    fn from(value: DhtOpLight) -> Self {
        let t = value.get_type();
        Self(value.action_hash().clone().into(), t)
    }
}

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
    AwaitingValidationDeps {
        by: NodeId,
        op: OpAction,
        deps: Vec<AnyDhtHash>,
    },
    Fetched {
        by: NodeId,
        op: OpAction,
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
            Step::AwaitingValidationDeps { by, op, deps } => f.write_fmt(format_args!(
                "[{}] Awaiting validation dependencies: {:?} deps: {:#?}",
                by, op, deps
            )),
            Step::Fetched { by, op } => f.write_fmt(format_args!("[{}] Fetched: {:?}", by, op)),
        }
    }
}

impl aitia::Fact for Step {
    type Context = Context;

    fn explain(&self, ctx: &Self::Context) -> String {
        self.to_string()
    }

    fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>> {
        use Step::*;
        match self.clone() {
            Authored { by, action } => None,
            Published { by, op } => Some(Integrated { by, op }.into()),
            Integrated { by, op } => Some(AppValidated { by, op }.into()),
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),
            SysValidated { by, op } => {
                let current = Cause::Any(vec![
                    Fetched {
                        by: by.clone(),
                        op: op.clone(),
                    }
                    .into(),
                    Authored {
                        by: by.clone(),
                        action: op.0.clone(),
                    }
                    .into(),
                ]);

                let dep = ctx.sysval_dep(&op);
                let mut causes = vec![current];
                causes.extend(
                    dep.map(|OpAction(action, _)| {
                        Cause::from(Integrated {
                            by,
                            op: OpAction(action.clone(), DhtOpType::StoreRecord),
                        })
                    })
                    .into_iter(),
                );

                Some(Cause::Every(causes))
            }
            AwaitingValidationDeps { by, op, deps } => {
                let causes = deps
                    .into_iter()
                    .map(|op| Cause::from(Integrated { by: by.clone(), op }))
                    .collect();
                Some(Cause::Every(causes))
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
        }
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}
