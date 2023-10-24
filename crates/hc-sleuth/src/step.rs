use crate::*;
use holochain_state::{prelude::named_params, validation_db::ValidationStage};

pub type OpRef = (ActionHash, DhtOpType);

#[derive(Clone, PartialEq, Eq, std::fmt::Debug, std::hash::Hash)]
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

impl std::fmt::Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Authored { by, action } => {
                f.write_fmt(format_args!("Op Authored by node {}: {}", by, action))
            }
            Step::Published { by, op } => {
                f.write_fmt(format_args!("Op Published by node {}: {:?}", by, op))
            }
            Step::Integrated { by, op } => {
                f.write_fmt(format_args!("Op Integrated by node {}: {:?}", by, op))
            }
            Step::AppValidated { by, op } => {
                f.write_fmt(format_args!("Op AppValidated by node {}: {:?}", by, op))
            }
            Step::SysValidated { by, op } => {
                f.write_fmt(format_args!("Op SysValidated by node {}: {:?}", by, op))
            }
            Step::Fetched { by, op } => {
                f.write_fmt(format_args!("Op Fetched by node {}: {:?}", by, op))
            }
        }
    }
}

impl aitia::Fact for Step {
    type Context = crate::Context;

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
            SysValidated { by, op } => Some(aitia::Cause::Any(vec![
                Fetched { by, op: op.clone() }.into(),
                Authored { by, action: op.0 }.into(),
            ])),
            Fetched { by, op } => {
                let mut others: Vec<_> = (0..ctx.nodes.len())
                    .filter(|i| *i != by)
                    .map(|i| {
                        Published {
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
                let env = ctx.nodes.envs.get(by).unwrap();
                env.authored.test_read(move |txn| {
                    txn.query_row(
                        "SELECT rowid FROM Action WHERE author = :author AND hash = :hash",
                        named_params! {
                            ":author": by,
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
                let env = ctx.nodes.envs.get(by).unwrap();
                env.dht.test_read(move |txn| {
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
                let env = ctx.nodes.envs.get(by).unwrap();
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
                let env = ctx.nodes.envs.get(by).unwrap();
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
                let env = ctx.nodes.envs.get(by).unwrap();
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
                let env = ctx.nodes.envs.get(by).unwrap();
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
