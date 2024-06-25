//! # Validation Database Types

use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::rusqlite::ToSql;

/// The status of a [`DhtOp`](holochain_types::dht_op::DhtOp) in limbo
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ValidationStage {
    /// Is awaiting to be system validated
    Pending,
    /// Is waiting for dependencies so the op can proceed to system validation
    AwaitingSysDeps,
    /// Is awaiting to be app validated
    SysValidated,
    /// Is waiting for dependencies so the op can proceed to app validation
    AwaitingAppDeps,
    /// Is awaiting to be integrated.
    AwaitingIntegration,
}

impl ToSql for ValidationStage {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        let stage = match self {
            ValidationStage::Pending => None,
            ValidationStage::AwaitingSysDeps => Some(0),
            ValidationStage::SysValidated => Some(1),
            ValidationStage::AwaitingAppDeps => Some(2),
            ValidationStage::AwaitingIntegration => Some(3),
        };
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            stage.into(),
        ))
    }
}
