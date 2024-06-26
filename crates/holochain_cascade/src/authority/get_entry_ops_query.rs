use std::sync::Arc;

use holo_hash::EntryHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::prelude::*;
use holochain_state::query::StateQueryError;

#[derive(Debug, Clone)]
pub struct GetEntryOpsQuery(EntryHash);

impl GetEntryOpsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

pub struct Item {
    op_type: ChainOpType,
    action: SignedAction,
}

#[derive(Debug, Default)]
pub struct State {
    ops: WireEntryOps,
    entry_data: Option<(EntryHash, EntryType)>,
}

impl Query for GetEntryOpsQuery {
    type Item = Judged<Item>;
    type State = State;
    type Output = WireEntryOps;

    fn query(&self) -> String {
        "
        SELECT Action.blob AS action_blob, DhtOp.type AS dht_type,
        DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Action On DhtOp.action_hash = Action.hash
        WHERE DhtOp.type IN (:store_entry, :delete, :update)
        AND
        DhtOp.basis_hash = :entry_hash
        AND
        DhtOp.when_integrated IS NOT NULL
        "
        .into()
    }

    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":store_entry": ChainOpType::StoreEntry,
            ":delete": ChainOpType::RegisterDeletedEntryAction,
            ":update": ChainOpType::RegisterUpdatedContent,
            ":entry_hash": self.0,
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
        Ok(Default::default())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            ChainOpType::StoreEntry => {
                if dht_op
                    .data
                    .action
                    .entry_type()
                    .filter(|et| *et.visibility() == EntryVisibility::Public)
                    .is_some()
                {
                    let status = dht_op.validation_status();
                    if state.entry_data.is_none() {
                        state.entry_data = dht_op
                            .data
                            .action
                            .entry_data()
                            .map(|(h, t)| (h.clone(), t.clone()));
                    }
                    state
                        .ops
                        .creates
                        .push(Judged::raw(dht_op.data.action.try_into()?, status));
                }
            }
            ChainOpType::RegisterDeletedEntryAction => {
                let status = dht_op.validation_status();
                state
                    .ops
                    .deletes
                    .push(Judged::raw(dht_op.data.action.try_into()?, status));
            }
            ChainOpType::RegisterUpdatedContent => {
                let status = dht_op.validation_status();
                let action = dht_op.data.action;
                state.ops.updates.push(Judged::raw(
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
        if let Some((entry_hash, entry_type)) = state.entry_data {
            let entry = stores.get_entry(&entry_hash)?;
            state.ops.entry = entry.map(|entry| EntryData { entry, entry_type });
        }
        Ok(state.ops)
    }
}
