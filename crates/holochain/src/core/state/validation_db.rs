//! # Validation Database Types

use holo_hash::{AnyDhtHash, DhtOpHash};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvBufFresh,
    db::VALIDATION_LIMBO,
    db_fixture::DbFixture,
    db_fixture::LoadDbFixture,
    error::DatabaseResult,
    prelude::Readable,
    prelude::{EnvironmentRead, GetDb},
};
use holochain_types::{dht_op::DhtOpLight, Timestamp};
use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
/// The database for putting ops into to await validation
pub struct ValidationLimboStore(pub KvBufFresh<ValidationLimboKey, ValidationLimboValue>);

/// Key to the validation limbo
pub type ValidationLimboKey = DhtOpHash;

/// A type for storing in databases that only need the hashes.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct ValidationLimboValue {
    /// Status of this op in the limbo
    pub status: ValidationLimboStatus,
    /// The actual op
    pub op: DhtOpLight,
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub enum ValidationLimboStatus {
    /// Is awaiting to be system validated
    Pending,
    /// Is waiting for dependencies so the op can proceed to system validation
    AwaitingSysDeps(AnyDhtHash),
    /// Is awaiting to be app validated
    SysValidated,
    /// Is waiting for dependencies so the op can proceed to app validation
    AwaitingAppDeps(Vec<AnyDhtHash>),
}

impl ValidationLimboStore {
    /// Create a new Validation Limbo db
    pub fn new(env: EnvironmentRead) -> DatabaseResult<Self> {
        let db = env.get_db(&*VALIDATION_LIMBO)?;
        Ok(Self(KvBufFresh::new(env, db)))
    }
}

impl LoadDbFixture for ValidationLimboStore {
    type FixtureItem = (ValidationLimboKey, ValidationLimboValue);

    fn write_test_datum(&mut self, datum: Self::FixtureItem) {
        self.0.write_test_datum(datum)
    }

    fn read_test_data<R: Readable>(&self, reader: &R) -> DbFixture<Self> {
        self.0.read_test_data(reader)
    }
}
