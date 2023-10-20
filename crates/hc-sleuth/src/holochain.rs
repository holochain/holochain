use crate::*;

mod action_authored;
mod action_integrated;
mod op_app_validated;
mod op_integrated;

pub use action_authored::*;
pub use action_integrated::*;
pub use op_app_validated::*;
pub use op_integrated::*;

pub type OpRef = (ActionHash, DhtOpType);

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct OpPublished {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for OpPublished {
    fn cause(&self, ctx: &Context) -> ACause {
        OpIntegrated::new(self.by.clone(), self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!("Node {} published Op {:?} at least once", self.by, self.op)
    }

    fn check(&self, ctx: &Context) -> bool {
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self {
            by,
            op: (action_hash, op_type),
        } = self.clone();
        env.dht.test_read(move |txn| {
            txn.query_row(
                "SELECT last_publish_time FROM DhtOp 
                WHERE action_hash = :action_hash 
                  AND type = :type",
                named_params! {
                    ":action_hash": action_hash,
                    ":type": op_type,
                },
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap()
            .is_some()
        })
    }
}

#[derive(Debug, Clone, derive_more::Constructor)]
pub struct OpIntegrated {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for OpIntegrated {
    fn cause(&self, ctx: &Context) -> ACause {
        OpAppValidated::new(self.by.clone(), self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!(
            "Action {} is integrated wrt OpType {} by node {}",
            self.op.0, self.op.1, self.by
        )
    }

    fn check(&self, ctx: &Context) -> bool {
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self {
            by,
            op: (action_hash, op_type),
        } = self.clone();
        env.dht.test_read(move |txn| {
            txn.query_row(
                "SELECT when_integrated FROM DhtOp 
                WHERE action_hash = :action_hash 
                  AND type = :type",
                named_params! {
                    ":action_hash": action_hash,
                    ":type": op_type,
                },
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap()
            .is_some()
        })
    }
}

#[derive(Debug, Clone, derive_more::Constructor)]
pub struct OpAppValidated {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for OpAppValidated {
    fn cause(&self, ctx: &Context) -> ACause {
        OpSysValidated::new(self.by.clone(), self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!(
            "Action {} is app validated wrt OpType {} by node {}",
            self.op.0, self.op.1, self.by
        )
    }

    fn check(&self, ctx: &Context) -> bool {
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self {
            by,
            op: (action_hash, op_type),
        } = self.clone();
        env.dht.test_read(move |txn| {
            txn.query_row(
                "SELECT rowid FROM DhtOp 
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
}

#[derive(Debug, Clone, derive_more::Constructor)]
pub struct OpSysValidated {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for OpSysValidated {
    fn cause(&self, ctx: &Context) -> ACause {
        any![
            ActionAuthored::new(self.by.clone(), self.op.0.clone()),
            OpFetched::new(self.by.clone(), self.op.clone())
        ]
    }

    fn explain(&self) -> String {
        format!(
            "Action {} is sys validated wrt OpType {} by node {}",
            self.op.0, self.op.1, self.by
        )
    }

    fn check(&self, ctx: &Context) -> bool {
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self {
            by,
            op: (action_hash, op_type),
        } = self.clone();
        env.dht.test_read(move |txn| {
            txn.query_row(
                "SELECT rowid FROM DhtOp
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
}

#[derive(Debug, Clone, derive_more::Constructor)]
pub struct OpFetched {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for OpFetched {
    fn cause(&self, ctx: &Context) -> ACause {
        let mut causes = vec![];
        for n in 0..ctx.nodes.len() {
            causes.push(OpPublished::new(n, self.op.clone()).into());
        }
        // for n in 0..ctx.nodes.len() {
        //     causes.push(OpGossiped(n, self.op.clone()).into());
        // }
        ACause::new(Any::new(causes))
    }

    fn explain(&self) -> String {
        format!("Op was not fetched")
    }

    fn check(&self, ctx: &Context) -> bool {
        // TODO: should do a check involving the actual FetchPool
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self {
            by,
            op: (action_hash, op_type),
        } = self.clone();
        env.dht.test_read(move |txn| {
            txn.query_row(
                "SELECT rowid FROM DhtOp
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

/*
#[derive(Debug, derive_more::Constructor)]
pub struct GossipReceived {
    pub by: NodeId,
    pub op: OpRef,
}
impl Fact for GossipReceived {
    fn cause(&self, ctx: &Context) -> ACause {
        todo!()
    }

    fn explain(&self) -> String {
        format!("Op was not received via gossip")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!("this info is not available in Holochain databases yet")
    }
}

#[derive(Debug)]
pub struct PublishReceived {
    pub by: NodeId,
    pub from: NodeId,
    pub op: OpRef,
}
impl Fact for PublishReceived {
    fn cause(&self, ctx: &Context) -> ACause {
        OpPublished::new(self.from.clone(), self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!("Op {:?} was received via publish by {}", self.op, self.by)
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!("this info is not available in Holochain databases yet")
    }
}
*/
