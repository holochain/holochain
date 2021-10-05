use holo_hash::HeaderHash;
use holochain_p2p::event::GetOptions;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::prelude::*;
use holochain_state::query::StateQueryError;
use holochain_types::dht_op::DhtOpType;
use holochain_types::element::WireElementOps;
use holochain_types::header::WireUpdateRelationship;
use holochain_zome_types::HasValidationStatus;
use holochain_zome_types::Judged;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::TryFrom;
use holochain_zome_types::TryInto;

#[derive(Debug, Clone)]
pub struct GetElementOpsQuery(HeaderHash, GetOptions);

impl GetElementOpsQuery {
    pub fn new(hash: HeaderHash, request: GetOptions) -> Self {
        Self(hash, request)
    }
}

pub struct Item {
    op_type: DhtOpType,
    header: SignedHeader,
}

impl Query for GetElementOpsQuery {
    type Item = Judged<Item>;
    type State = WireElementOps;
    type Output = Self::State;

    fn query(&self) -> String {
        let request_type = self.1.request_type.clone();
        let query = "
            SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
            DhtOp.validation_status AS status
            FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type IN (:store_element, :delete, :update)
            AND
            DhtOp.basis_hash = :header_hash
        ";
        let is_integrated = "
            AND
            DhtOp.when_integrated IS NOT NULL
        ";
        match request_type {
            holochain_p2p::event::GetRequest::All
            | holochain_p2p::event::GetRequest::Content
            | holochain_p2p::event::GetRequest::Metadata => {
                format!("{}{}", query, is_integrated)
            }
            holochain_p2p::event::GetRequest::Pending => query.into(),
        }
    }

    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":store_element": DhtOpType::StoreElement,
            ":delete": DhtOpType::RegisterDeletedBy,
            ":update": DhtOpType::RegisterUpdatedElement,
            ":header_hash": self.0,
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
        Ok(WireElementOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Item) -> StateQueryResult<Self::State> {
        match &dht_op.data.op_type {
            DhtOpType::StoreElement => {
                if state.header.is_none() {
                    state.header = Some(dht_op.map(|d| d.header));
                } else {
                    // TODO: This is weird there are multiple store elements ops for the same header??
                }
            }
            DhtOpType::RegisterDeletedBy => {
                let status = dht_op.validation_status();
                state
                    .deletes
                    .push(Judged::raw(dht_op.data.header.try_into()?, status));
            }
            DhtOpType::RegisterUpdatedElement => {
                let status = dht_op.validation_status();
                let header = dht_op.data.header;
                state.updates.push(Judged::raw(
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
        let entry_hash = state.header.as_ref().and_then(|wire_op| {
            wire_op
                .data
                .0
                .entry_data()
                .map(|(hash, et)| (hash, et.visibility()))
        });
        if let Some((entry_hash, holochain_zome_types::EntryVisibility::Public)) = entry_hash {
            let entry = stores.get_entry(entry_hash)?;
            state.entry = entry;
        }
        Ok(state)
    }
}
