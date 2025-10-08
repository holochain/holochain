use holo_hash::ActionHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GetRecordOpsQuery(ActionHash);

impl GetRecordOpsQuery {
    pub fn new(hash: ActionHash) -> Self {
        Self(hash)
    }
}

pub struct Item {
    op_type: ChainOpType,
    action: SignedAction,
}

impl Query for GetRecordOpsQuery {
    type Item = Judged<Item>;
    type State = WireRecordOps;
    type Output = Self::State;

    fn query(&self) -> String {
        "
            SELECT Action.blob AS action_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
            JOIN Action On DhtOp.action_hash = Action.hash
            WHERE DhtOp.type IN (:store_record, :delete, :update)
            AND
            DhtOp.basis_hash = :action_hash
            AND
            DhtOp.when_integrated IS NOT NULL
        "
        .into()
    }

    fn params(&self) -> Vec<Params<'_>> {
        let params = named_params! {
            ":store_record": ChainOpType::StoreRecord,
            ":delete": ChainOpType::RegisterDeletedBy,
            ":update": ChainOpType::RegisterUpdatedRecord,
            ":action_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let action =
                from_blob::<SignedAction>(row.get(row.as_ref().column_index("action_blob")?)?)?;
            let op_type = row.get(row.as_ref().column_index("dht_type")?)?;
            let validation_status = row.get(row.as_ref().column_index("status")?)?;
            Ok(Judged::raw(Item { op_type, action }, validation_status))
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(WireRecordOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            ChainOpType::StoreRecord => {
                if state.action.is_none() {
                    state.action = Some(dht_op.map(|d| d.action));
                }
            }
            ChainOpType::RegisterDeletedBy => {
                let status = dht_op.validation_status();
                state
                    .deletes
                    .push(Judged::raw(dht_op.data.action.try_into()?, status));
            }
            ChainOpType::RegisterUpdatedRecord => {
                let status = dht_op.validation_status();
                let action = dht_op.data.action;
                state.updates.push(Judged::raw(
                    WireUpdateRelationship::try_from(action)?,
                    status,
                ));
            }
            _ => return Err(StateQueryError::UnexpectedOp(dht_op.data.op_type)),
        }
        Ok(state)
    }

    fn render<S>(&self, mut state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let entry_hash = state.action.as_ref().and_then(|wire_op| {
            wire_op
                .data
                .entry_data()
                .map(|(hash, et)| (hash, et.visibility()))
        });
        if let Some((entry_hash, EntryVisibility::Public)) = entry_hash {
            let entry = stores.get_entry(entry_hash)?;
            state.entry = entry;
        }
        Ok(state)
    }
}
