//! # Validation Database Types

use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

/// The status of a [DhtOp] in limbo
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ValidationLimboStatus {
    /// Is awaiting to be system validated
    Pending,
    /// Is waiting for dependencies so the op can proceed to system validation
    AwaitingSysDeps(AnyDhtHash),
    /// Is awaiting to be app validated
    SysValidated,
    /// Is waiting for dependencies so the op can proceed to app validation
    AwaitingAppDeps(Vec<AnyDhtHash>),
    /// Is awaiting to be integrated.
    AwaitingIntegration,
}
