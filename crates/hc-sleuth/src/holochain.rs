use crate::*;

mod action_integrated;
mod op_app_validated;
mod op_integrated;

pub use action_integrated::*;
pub use op_app_validated::*;
pub use op_integrated::*;

pub type OpRef = (ActionHash, DhtOpType);

#[derive(Debug, derive_more::Constructor)]
pub struct ActionIntegrated {
    op: OpRef,
}
impl Fact for ActionIntegrated {
    fn cause(&self) -> ACause {
        OpIntegrated::new(self.op.clone()).into()
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }

    fn explain(&self) -> String {
        format!(
            "Action {} is not integrated wrt OpType {}",
            self.op.0, self.op.1
        )
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct OpIntegrated {
    op: OpRef,
}
impl Fact for OpIntegrated {
    fn cause(&self) -> ACause {
        OpAppValidated::new(self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!(
            "Op {} is not integrated wrt OpType {}",
            self.op.0, self.op.1
        )
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct OpAppValidated {
    op: OpRef,
}
impl Fact for OpAppValidated {
    fn cause(&self) -> ACause {
        OpSysValidated::new(self.op.clone()).into()
    }

    fn explain(&self) -> String {
        format!("Op is not app validated")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct OpSysValidated {
    op: OpRef,
}
impl Fact for OpSysValidated {
    fn cause(&self) -> ACause {
        any![
            ActionAuthored::new(self.op.0.clone()),
            OpFetched::new(self.op.clone())
        ]
    }

    fn explain(&self) -> String {
        format!("Op is not sys validated")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct OpFetched {
    op: OpRef,
}
impl Fact for OpFetched {
    fn cause(&self) -> ACause {
        any![
            PublishReceived::new(self.op.clone()),
            GossipReceived::new(self.op.clone())
        ]
    }

    fn explain(&self) -> String {
        format!("Op was not fetched")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct GossipReceived {
    op: OpRef,
}
impl Fact for GossipReceived {
    fn cause(&self) -> ACause {
        todo!()
    }

    fn explain(&self) -> String {
        format!("Op was not received via gossip")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct PublishReceived {
    op: OpRef,
}
impl Fact for PublishReceived {
    fn cause(&self) -> ACause {
        todo!()
    }

    fn explain(&self) -> String {
        format!("Op was not received via publish")
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct ActionAuthored {
    action: ActionHash,
}
impl Fact for ActionAuthored {
    fn cause(&self) -> ACause {
        ().into()
    }

    fn explain(&self) -> String {
        format!("Action {} was never authored", self.action)
    }

    fn check(&self, ctx: &Context) -> bool {
        todo!()
    }
}
