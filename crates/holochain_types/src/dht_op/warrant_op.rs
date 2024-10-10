use super::*;

impl From<WarrantOp> for DhtOp {
    fn from(op: WarrantOp) -> Self {
        DhtOp::WarrantOp(Box::new(op))
    }
}

impl From<SignedWarrant> for DhtOp {
    fn from(op: SignedWarrant) -> Self {
        DhtOp::WarrantOp(Box::new(WarrantOp::from(op)))
    }
}

impl From<WarrantOp> for DhtOpLite {
    fn from(op: WarrantOp) -> Self {
        DhtOpLite::Warrant(Box::new(op))
    }
}

impl From<SignedWarrant> for DhtOpLite {
    fn from(warrant: SignedWarrant) -> Self {
        DhtOpLite::Warrant(Box::new(warrant.into()))
    }
}
