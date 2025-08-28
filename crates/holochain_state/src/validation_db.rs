//! Validation database types and functions.

use crate::query::StateQueryResult;
use holo_hash::ActionHash;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::rusqlite::types::{FromSql, FromSqlResult, ValueRef};
use holochain_sqlite::rusqlite::{params, OptionalExtension, ToSql};
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::validate::ValidationStatus;

/// The status of a [`DhtOp`](holochain_types::dht_op::DhtOp) in limbo
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

impl FromSql for ValidationStage {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let stage: Option<i32> = FromSql::column_result(value)?;
        match stage {
            None => Ok(ValidationStage::Pending),
            Some(0) => Ok(ValidationStage::AwaitingSysDeps),
            Some(1) => Ok(ValidationStage::SysValidated),
            Some(2) => Ok(ValidationStage::AwaitingAppDeps),
            Some(3) => Ok(ValidationStage::AwaitingIntegration),
            Some(_) => Err(holochain_sqlite::rusqlite::types::FromSqlError::Other(
                Box::new(std::io::Error::other("Invalid ValidationStage value")),
            )),
        }
    }
}

/// Get the validation state of a [`DhtOp`] by its action hash and type.
///
/// Returns `Ok(None)` if the action is not found or there is no matching op type for the action.
/// Otherwise, returns a combination of stage and status:
///   - If the op has just arrived, it will return `(None, None)`.
///   - If the op is in the process of validation, it will return `(Some(stage), None)`.
///   - If a validation decision has been made but the op hasn't yet been integrated, it will return `(Some(stage), Some(status))`.
///   - If the op has been fully integrated, it will return `(None, Some(status))`.
pub async fn get_dht_op_validation_state(
    dht_db: &holochain_sqlite::db::DbRead<DbKindDht>,
    action_hash: ActionHash,
    op_type: ChainOpType,
) -> StateQueryResult<Option<(Option<ValidationStage>, Option<ValidationStatus>)>> {
    dht_db.read_async(move |txn| -> StateQueryResult<_> {
        let mut stmt = txn.prepare(
            "SELECT validation_stage, validation_status FROM DhtOp WHERE action_hash = ? AND type = ?"
        )?;

        Ok(stmt.query_row(params![action_hash, op_type], |row| -> Result<(Option<ValidationStage>, Option<ValidationStatus>), _> {
            let stage = row.get::<_, Option<ValidationStage>>(0)?;
            let state = row.get::<_, Option<ValidationStatus>>(1)?;
            Ok((stage, state))
        }).optional()?)
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutations::insert_op_dht;
    use crate::prelude::{
        set_validation_stage, set_validation_status, set_when_integrated, test_dht_db,
        StateMutationResult,
    };
    use fixt::fixt;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::fixt::SignatureFixturator;
    use holochain_zome_types::fixt::ActionFixturator;
    use holochain_zome_types::prelude::Timestamp;
    use test_case::test_case;

    #[test_case(None, None ; "just received, no status yet")]
    #[test_case(Some(ValidationStage::AwaitingSysDeps), None ; "waiting for sys deps, no status yet")]
    #[test_case(Some(ValidationStage::SysValidated), None ; "sys validated, no status yet")]
    #[test_case(Some(ValidationStage::AwaitingAppDeps), None ; "waiting for app deps, no status yet")]
    #[test_case(Some(ValidationStage::AwaitingIntegration), Some(ValidationStatus::Valid) ; "waiting for integration, valid")]
    #[test_case(Some(ValidationStage::AwaitingIntegration), Some(ValidationStatus::Rejected) ; "waiting for integration, rejected")]
    #[test_case(None, Some(ValidationStatus::Valid) ; "integrated, valid")]
    #[tokio::test(flavor = "multi_thread")]
    async fn all_get_op_validation_state(
        test_stage: Option<ValidationStage>,
        test_validation_status: Option<ValidationStatus>,
    ) {
        let db = test_dht_db();

        let op = DhtOp::from(ChainOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));

        db.write_async({
            let op = op.clone();
            let op_hashed = DhtOpHashed::from_content_sync(op);
            let test_stage = test_stage.clone();
            let test_validation_status = test_validation_status;
            move |txn| -> StateMutationResult<()> {
                insert_op_dht(txn, &op_hashed, 0, None)?;

                if let Some(stage) = test_stage {
                    set_validation_stage(txn, &op_hashed.hash, stage)?;
                }

                if let Some(status) = test_validation_status {
                    set_validation_status(txn, &op_hashed.hash, status)?;
                    set_when_integrated(txn, &op_hashed.hash, Timestamp::now())?;
                }

                Ok(())
            }
        })
        .await
        .unwrap();

        let (stage, state) = get_dht_op_validation_state(
            &db,
            ActionHash::with_data_sync(&op.as_chain_op().unwrap().action()),
            op.as_chain_op().unwrap().get_type(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(test_stage, stage);
        assert_eq!(test_validation_status, state);
    }
}
