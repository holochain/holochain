//! # Validation Database Types

use holo_hash::{AnyDhtHash, DhtOpHash};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvBuf, db::VALIDATION_LIMBO, error::DatabaseResult, prelude::GetDb, transaction::Reader,
};
use holochain_types::{dht_op::DhtOp, Timestamp};
use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
/// The database for putting ops into to await validation
pub struct ValidationLimboStore<'env>(
    pub KvBuf<'env, ValidationLimboKey, ValidationLimboValue, Reader<'env>>,
);

/// Key to the validation limbo
pub type ValidationLimboKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ValidationLimboValue {
    /// Status of this op in the limbo
    pub status: ValidationLimboStatus,
    /// The actual op
    pub op: DhtOp,
    /// Where the op was sent to
    pub basis: AnyDhtHash,
    /// When the op was added to limbo
    pub time_added: Timestamp,
    /// Last time we tried to validated the op
    pub last_try: Option<Timestamp>,
    /// Number of times we have tried to validate the op
    pub num_tries: u32,
}

/// The status of a [DhtOp] in limbo
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ValidationLimboStatus {
    /// Is awaiting to be system validated
    Pending,
    /// Is waiting for dependencies so the op can proceed to system validation
    AwaitingSysDeps,
    /// Is awaiting to be app validated
    SysValidated,
    /// Is waiting for dependencies so the op can proceed to app validation
    AwaitingAppDeps,
}

impl<'env> ValidationLimboStore<'env> {
    /// Create a new Validation Limbo db
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let db = dbs.get_db(&*VALIDATION_LIMBO)?;
        Ok(Self(KvBuf::new(reader, db)?))
    }
}
