use std::sync::Arc;

use crate::*;

pub trait Fact: std::fmt::Debug {
    fn cause(&self) -> ACause;
    fn check(&self) -> bool;
    fn report(&self) -> String;
}

#[derive(Debug, derive_more::Deref)]
pub struct AFact(Arc<dyn Fact>);

impl Clone for AFact {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl AFact {
    pub fn new(f: impl Fact + 'static) -> Self {
        Self(Arc::new(f))
    }
}

impl<F: Fact> Cause for F {
    fn backtrack(&self) -> Report {
        let pass = self.check();
        if pass {
            // Terminate backtracking as soon as a passing check is reached
            vec![]
        } else {
            // Add this fact to the path
            let mut report = self.cause().backtrack();
            report.push(self.report());
            report
        }
    }
}

impl Fact for () {
    fn cause(&self) -> ACause {
        ().into()
    }

    fn check(&self) -> bool {
        true
    }

    fn report(&self) -> String {
        unreachable!()
    }
}

pub type OpRef = (ActionHash, DhtOpType);

#[derive(Debug, derive_more::Constructor)]
pub struct ActionIntegrated {
    op: OpRef,
}
impl Fact for ActionIntegrated {
    fn cause(&self) -> ACause {
        OpIntegrated::new(self.op.clone()).into()
    }

    fn check(&self) -> bool {
        todo!()
    }

    fn report(&self) -> String {
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

    fn report(&self) -> String {
        format!(
            "Op {} is not integrated wrt OpType {}",
            self.op.0, self.op.1
        )
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Op is not app validated")
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Op is not sys validated")
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Op was not fetched")
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Op was not received via gossip")
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Op was not received via publish")
    }

    fn check(&self) -> bool {
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

    fn report(&self) -> String {
        format!("Action {} was never authored", self.action)
    }

    fn check(&self) -> bool {
        todo!()
    }
}
