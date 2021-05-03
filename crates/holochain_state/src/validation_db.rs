//! # Validation Database Types

use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::DhtOpHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::buffer::KvBufFresh;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::GetTable;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use shrinkwraprs::Shrinkwrap;

// #[derive(Shrinkwrap)]
// #[shrinkwrap(mutable)]
// /// The database for putting ops into to await validation
// pub struct ValidationLimboStore(pub KvBufFresh<ValidationLimboKey, ValidationLimboValue>);

// /// Key to the validation limbo
// pub type ValidationLimboKey = DhtOpHash;

// /// A type for storing in databases that only need the hashes.
// #[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
// pub struct ValidationLimboValue {
//     /// Status of this op in the limbo
//     pub status: ValidationLimboStatus,
//     /// The actual op
//     pub op: DhtOpLight,
//     /// Where the op was sent to
//     pub basis: AnyDhtHash,
//     /// When the op was added to limbo
//     pub time_added: Timestamp,
//     /// Last time we tried to validated the op
//     pub last_try: Option<Timestamp>,
//     /// Number of times we have tried to validate the op
//     pub num_tries: u32,
//     /// The agent that sent you this op
//     pub from_agent: Option<AgentPubKey>,
//     /// Send a receipt to this author.
//     pub send_receipt: bool,
// }

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

// impl ValidationLimboStore {
//     /// Create a new Validation Limbo db
//     pub fn new(env: EnvRead) -> DatabaseResult<Self> {
//         let db = env.get_table(TableName::ValidationLimbo)?;
//         Ok(Self(KvBufFresh::new(env, db)))
//     }
// }
