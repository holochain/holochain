use holo_hash::ActionHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::prelude::*;
use std::sync::Arc;

/// NB: If this query is ever used for multiple stores, instead of only the DHT store,
/// the last matching ChainOp found in the stores is returned as the result. Matches from
/// previous stores are overwritten.
#[derive(Debug, Clone)]
pub struct GetChainOpByTypeQuery(ActionHash, ChainOpType);

impl GetChainOpByTypeQuery {
    pub fn new(hash: ActionHash, op_type: ChainOpType) -> Self {
        Self(hash, op_type)
    }
}

impl Query for GetChainOpByTypeQuery {
    type Item = Judged<ChainOp>;
    type State = WireMaybeOpByType;
    type Output = Self::State;

    fn query(&self) -> String {
        "
            SELECT
                Action.blob AS action_blob,
                Entry.blob AS entry_blob,
                DhtOp.validation_status AS validation_status
            FROM
                DhtOp
                JOIN
                    Action ON DhtOp.action_hash = Action.hash
                    LEFT JOIN
                        Entry ON Action.entry_hash = Entry.hash
            WHERE
                DhtOp.action_hash = :action_hash
            AND
                DhtOp.type = :op_type
            AND
                DhtOp.when_integrated IS NOT NULL
        "
        .into()
    }

    fn params(&self) -> Vec<Params<'_>> {
        let params = named_params! {
            ":action_hash": self.0,
            ":op_type": self.1,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let op_type = self.1.clone();
        let f = move |row: &Row| {
            let action =
                from_blob::<SignedAction>(row.get(row.as_ref().column_index("action_blob")?)?)?;
            let maybe_entry =
                row.get::<_, Option<Vec<u8>>>(row.as_ref().column_index("entry_blob")?)?;
            let maybe_entry = match maybe_entry {
                Some(blob) => Some(from_blob::<Entry>(blob)?),
                None => None,
            };
            let chain_op = ChainOp::from_type(op_type, action, maybe_entry)?;
            let validation_status = row.get(row.as_ref().column_index("validation_status")?)?;
            Ok(Judged::raw(chain_op, validation_status))
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(None)
    }

    fn fold(
        &self,
        _state: Self::State,
        judged_chain_op: Self::Item,
    ) -> StateQueryResult<Self::State> {
        Ok(Some(WireOpByType(judged_chain_op)))
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state)
    }
}
