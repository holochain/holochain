use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::*;
use holochain_state::{prelude::*, validation_db::ValidationStage};

pub type OpRef = (ActionHash, DhtOpType);

#[derive(Clone, PartialEq, Eq, std::fmt::Debug, std::hash::Hash)]
pub enum Step<NodeId: aitia::Fact> {
    Authored { by: NodeId, action: ActionHash },
    Published { by: NodeId, op: OpRef },
    Integrated { by: NodeId, op: OpRef },
    AppValidated { by: NodeId, op: OpRef },
    SysValidated { by: NodeId, op: OpRef },
    Fetched { by: NodeId, op: OpRef },
    // GossipReceived {},
    // PublishReceived {},
}

impl<NodeId: aitia::Fact> std::fmt::Display for Step<NodeId> {
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

impl<NodeId: aitia::Fact> aitia::Fact for Step<NodeId> {
    type Context = crate::Context<NodeId>;

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
                let env = ctx.nodes.envs.get(&by).unwrap();

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

                let dep = env
                    .integrated(move |txn| {
                        Ok(txn
                            .query_row(
                                "
                        SELECT dependency FROM DhtOp
                        WHERE action_hash = :action_hash 
                          AND type = :type",
                                named_params! {
                                    ":action_hash": op.0,
                                    ":type": op.1,
                                },
                                |row| row.get::<_, Option<ActionHash>>(0),
                            )
                            .optional()?
                            .flatten())
                    })
                    .unwrap();
                let mut causes = vec![current];
                causes.extend(
                    dep.map(|action| {
                        aitia::Cause::from(Integrated {
                            by,
                            op: (action, DhtOpType::StoreRecord),
                        })
                    })
                    .into_iter(),
                );

                Some(aitia::Cause::Every(causes))
            }
            Fetched { by, op } => {
                let mut others: Vec<_> = ctx
                    .nodes
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
                Some(aitia::Cause::Any(others))
            }
        }
    }

    fn check(&self, ctx: &Self::Context) -> bool {
        match self.clone() {
            Step::Authored { by, action } => {
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.authored.test_read(move |txn| {
                    txn.query_row(
                        "SELECT rowid FROM Action WHERE hash = :hash",
                        named_params! {
                            ":hash": action,
                        },
                        |row| row.get::<_, usize>(0),
                    )
                    .optional()
                    .unwrap()
                    .is_some()
                })
            }
            Step::Published {
                by,
                op: (action_hash, op_type),
            } => {
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.authored.test_read(move |txn| {
                    txn.query_row(
                        "
                        SELECT last_publish_time FROM DhtOp
                        WHERE action_hash = :action_hash 
                          AND type = :type",
                        named_params! {
                            ":action_hash": action_hash,
                            ":type": op_type,
                        },
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .unwrap()
                    .flatten()
                    .is_some()
                })
            }
            Step::Integrated {
                by,
                op: (action_hash, op_type),
            } => {
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.dht.test_read(move |txn| {
                    txn.query_row(
                        "
                        SELECT when_integrated FROM DhtOp 
                        WHERE action_hash = :action_hash 
                          AND type = :type",
                        named_params! {
                            ":action_hash": action_hash,
                            ":type": op_type,
                        },
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .unwrap()
                    .flatten()
                    .is_some()
                })
            }
            Step::AppValidated {
                by,
                op: (action_hash, op_type),
            } => {
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.dht.test_read(move |txn| {
                    txn.query_row(
                        "
                        SELECT rowid FROM DhtOp 
                        WHERE action_hash = :action_hash 
                          AND type = :type 
                          AND validation_stage >= :stage
                        ",
                        named_params! {
                            ":action_hash": action_hash,
                            ":type": op_type,
                            ":stage": ValidationStage::AwaitingIntegration
                        },
                        |row| row.get::<_, usize>(0),
                    )
                    .optional()
                    .unwrap()
                    .is_some()
                })
            }
            Step::SysValidated {
                by,
                op: (action_hash, op_type),
            } => {
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.dht.test_read(move |txn| {
                    txn.query_row(
                        "
                        SELECT rowid FROM DhtOp
                        WHERE action_hash = :action_hash 
                          AND type = :type 
                          AND validation_stage >= :stage
                        ",
                        named_params! {
                            ":action_hash": action_hash,
                            ":type": op_type,
                            ":stage": ValidationStage::SysValidated
                        },
                        |row| row.get::<_, usize>(0),
                    )
                    .optional()
                    .unwrap()
                    .is_some()
                })
            }
            Step::Fetched {
                by,
                op: (action_hash, op_type),
            } => {
                // TODO: should do a check involving the actual FetchPool
                let env = ctx.nodes.envs.get(&by).unwrap();
                env.dht.test_read(move |txn| {
                    txn.query_row(
                        "
                        SELECT rowid FROM DhtOp
                        WHERE action_hash = :action_hash 
                          AND type = :type
                        ",
                        named_params! {
                            ":action_hash": action_hash,
                            ":type": op_type,
                        },
                        |row| row.get::<_, usize>(0),
                    )
                    .optional()
                    .unwrap()
                    .is_some()
                })
            }
        }
    }
}
