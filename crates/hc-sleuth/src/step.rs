use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::*;
use aitia::logging::FactLogTraits;
use aitia::{Cause, FactTraits};
use holochain_state::{prelude::*, validation_db::ValidationStage};

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
        op: OpLite,
    },
    Integrated {
        by: NodeId,
        op: OpLite,
    },
    AppValidated {
        by: NodeId,
        op: OpLite,
    },
    SysValidated {
        by: NodeId,
        op: OpLite,
    },
    PendingSysValidation {
        by: NodeId,
        op: OpLite,
        dep: Option<AnyDhtHash>,
    },
    PendingAppValidation {
        by: NodeId,
        op: OpLite,
        deps: Vec<AnyDhtHash>,
    },
    Fetched {
        by: NodeId,
        op: OpLite,
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
            PendingAppValidation { by, op, deps: _ } => Some(Cause::from(SysValidated { by, op })),
            SysValidated { by, op } => {
                let authored = Authored {
                    by: by.clone(),
                    action: op.action_hash().clone(),
                };

                let fetched = Fetched {
                    by: by.clone(),
                    op: op.clone(),
                };

                let received = Cause::Any(vec![authored.into(), fetched.into()]);
                let mut causes = vec![received];

                let dep = ctx.sysval_op_dep(&op).cloned();
                let pending = PendingSysValidation {
                    by: by.clone(),
                    op: op.clone(),
                    dep: dep.clone().map(|d| d.fetch_dependency_hash()),
                }
                .into();

                if let Some(dep_integrated) = dep.map(|op| Cause::from(Integrated { by, op })) {
                    Some(Cause::Every(vec![pending, dep_integrated]))
                } else {
                    Some(pending)
                }
            }
            PendingSysValidation { by, op, dep: _ } => {
                let authored = Authored {
                    by: by.clone(),
                    action: op.action_hash().clone(),
                };

                let fetched = Fetched {
                    by: by.clone(),
                    op: op.clone(),
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
        }
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}
