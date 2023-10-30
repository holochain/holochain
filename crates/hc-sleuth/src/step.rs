use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::*;
use aitia::logging::FactLogTraits;
use aitia::FactTraits;
use holochain_state::{prelude::*, validation_db::ValidationStage};

pub type OpRef = (ActionHash, DhtOpType);

pub type NodeId = String;

#[derive(
    Clone, PartialEq, Eq, std::fmt::Debug, std::hash::Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Step {
    Authored { by: NodeId, action: ActionHash },
    Published { by: NodeId, op: OpRef },
    Integrated { by: NodeId, op: OpRef },
    AppValidated { by: NodeId, op: OpRef },
    SysValidated { by: NodeId, op: OpRef },
    Fetched { by: NodeId, op: OpRef },
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
            Step::Fetched { by, op } => f.write_fmt(format_args!("[{}] Fetched: {:?}", by, op)),
        }
    }
}

impl aitia::Fact for Step {
    type Context = Context;

    fn explain(&self, ctx: &Self::Context) -> String {
        self.to_string()
    }

    fn cause(&self, ctx: &Self::Context) -> Option<aitia::Cause<Self>> {
        use Step::*;
        match self.clone() {
            Authored { by, action } => None,
            Published { by, op } => Some(Integrated { by, op }.into()),
            Integrated { by, op } => Some(AppValidated { by, op }.into()),
            AppValidated { by, op } => Some(SysValidated { by, op }.into()),
            SysValidated { by, op } => {
                let current = aitia::Cause::Any(vec![
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
                    dep.map(|(action, _)| {
                        aitia::Cause::from(Integrated {
                            by,
                            op: (action.clone(), DhtOpType::StoreRecord),
                        })
                    })
                    .into_iter(),
                );

                Some(aitia::Cause::Every(causes))
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
                Some(aitia::Cause::Any(others))
            }
        }
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        ctx.check(self)
    }
}
