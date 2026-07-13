//! Validation database types and functions.

use holochain_serialized_bytes::prelude::*;

/// The status of a [`DhtOp`](holochain_types::dht_v2::DhtOp) in limbo
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ValidationStage {
    /// Is awaiting system validation
    Pending,
    /// Is waiting for dependencies so the op can proceed to system validation
    AwaitingSysDeps,
    /// Is awaiting app validation
    SysValidated,
    /// Is waiting for dependencies so the op can proceed to app validation
    AwaitingAppDeps,
    /// Is awaiting integration
    AwaitingIntegration,
}
