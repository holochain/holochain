use std::sync::Arc;

use crate::*;

pub trait Fact: std::fmt::Debug {
    fn cause(&self) -> ACause;
    fn check(&self) -> bool;
}

#[derive(Clone, Debug, derive_more::Deref)]
pub struct AFact(Arc<dyn Fact>);

impl AFact {
    pub fn new(f: impl Fact + 'static) -> Self {
        Self(Arc::new(f))
    }
}

impl Cause for AFact {
    fn backtrack(&self) -> FactPath {
        let pass = self.check();
        if pass {
            // Terminate backtrack as soon as a passing check is reached
            vec![]
        } else {
            // Add this fact to the path
            let mut facts = self.cause().backtrack();
            facts.push(self.clone());
            facts
        }
    }
}

impl Fact for () {
    fn cause(&self) -> ACause {
        ().into()
    }

    fn check(&self) -> bool {
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
}

#[derive(Debug, derive_more::Constructor)]
pub struct OpIntegrated {
    op: OpRef,
}
impl Fact for OpIntegrated {
    fn cause(&self) -> ACause {
        OpAppValidated::new(self.op.clone()).into()
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

    fn check(&self) -> bool {
        todo!()
    }
}
