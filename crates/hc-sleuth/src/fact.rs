use std::sync::Arc;

use crate::*;

#[derive(Clone, derive_more::Deref)]
pub struct AFact(Arc<dyn Fact>);

impl<F: Fact + 'static> From<F> for AFact {
    fn from(f: F) -> Self {
        AFact(Arc::new(f))
    }
}

macro_rules! causes {
    ( $($c:expr),+ ) => {
        vec![$(AFact::from($c)),+]
    };
}

macro_rules! every {
    ( $($c:expr),+ ) => {
        AFact::from(Every(causes![$(($c)),+]))
    };
}

macro_rules! any {
    ( $($c:expr),+ ) => {
        AFact::from(Any(causes![$(($c)),+]))
    };
}

pub trait Fact {
    fn cause(&self) -> AFact;
    fn check(&self) -> anyhow::Result<bool>;
    fn traverse(&self) -> Vec<AFact> {
        self.cause().traverse()
    }
}

pub struct Every(Vec<AFact>);
pub struct Any(Vec<AFact>);

impl Fact for Any {
    fn cause(&self) -> AFact {
        todo!()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

pub type OpRef = (ActionHash, DhtOpType);

#[derive(derive_more::Constructor)]
pub struct ActionIntegrated {
    op: OpRef,
}
impl Fact for ActionIntegrated {
    fn cause(&self) -> AFact {
        OpIntegrated::new(self.op.clone()).into()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct OpIntegrated {
    op: OpRef,
}
impl Fact for OpIntegrated {
    fn cause(&self) -> AFact {
        OpAppValidated::new(self.op.clone()).into()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct OpAppValidated {
    op: OpRef,
}
impl Fact for OpAppValidated {
    fn cause(&self) -> AFact {
        OpSysValidated::new(self.op.clone()).into()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct OpSysValidated {
    op: OpRef,
}
impl Fact for OpSysValidated {
    fn cause(&self) -> AFact {
        any![
            ActionAuthored::new(self.op.0.clone()),
            OpFetched::new(self.op.clone())
        ]
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct OpFetched {
    op: OpRef,
}
impl Fact for OpFetched {
    fn cause(&self) -> AFact {
        any![
            PublishReceived::new(self.op.clone()),
            GossipReceived::new(self.op.clone())
        ]
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct GossipReceived {
    op: OpRef,
}
impl Fact for GossipReceived {
    fn cause(&self) -> AFact {
        todo!()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct PublishReceived {
    op: OpRef,
}
impl Fact for PublishReceived {
    fn cause(&self) -> AFact {
        todo!()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}

#[derive(derive_more::Constructor)]
pub struct ActionAuthored {
    action: ActionHash,
}
impl Fact for ActionAuthored {
    fn cause(&self) -> AFact {
        todo!()
    }

    fn check(&self) -> anyhow::Result<bool> {
        todo!()
    }
}
