use std::sync::Arc;

use holo_hash::EntryHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::dht_op::DhtOpType;
use holochain_types::header::WireUpdateRelationship;
use holochain_types::prelude::EntryData;
use holochain_types::prelude::HasValidationStatus;
use holochain_types::prelude::WireEntryOps;
use holochain_zome_types::EntryType;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::Judged;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::TryFrom;
use holochain_zome_types::TryInto;

#[derive(Debug, Clone)]
pub struct GetEntryOpsQuery(EntryHash);

impl GetEntryOpsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

pub struct Item {
    op_type: DhtOpType,
    header: SignedHeader,
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
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
        DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
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
            ":store_entry": DhtOpType::StoreEntry,
            ":delete": DhtOpType::RegisterDeletedEntryHeader,
            ":update": DhtOpType::RegisterUpdatedContent,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let header =
                from_blob::<SignedHeader>(row.get(row.as_ref().column_index("header_blob")?)?)?;
            let op_type = row.get(row.as_ref().column_index("dht_type")?)?;
            let validation_status = row.get(row.as_ref().column_index("status")?)?;
            Ok(Judged::raw(Item { op_type, header }, validation_status))
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Default::default())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            DhtOpType::StoreEntry => {
                if dht_op
                    .data
                    .header
                    .0
                    .entry_type()
                    .filter(|et| *et.visibility() == EntryVisibility::Public)
                    .is_some()
                {
                    let status = dht_op.validation_status();
                    if state.entry_data.is_none() {
                        state.entry_data = dht_op
                            .data
                            .header
                            .0
                            .entry_data()
                            .map(|(h, t)| (h.clone(), t.clone()));
                    }
                    state
                        .ops
                        .creates
                        .push(Judged::raw(dht_op.data.header.try_into()?, status));
                }
            }
            DhtOpType::RegisterDeletedEntryHeader => {
                let status = dht_op.validation_status();
                state
                    .ops
                    .deletes
                    .push(Judged::raw(dht_op.data.header.try_into()?, status));
            }
            DhtOpType::RegisterUpdatedContent => {
                let status = dht_op.validation_status();
                let header = dht_op.data.header;
                state.ops.updates.push(Judged::raw(
                    WireUpdateRelationship::try_from(header)?,
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
