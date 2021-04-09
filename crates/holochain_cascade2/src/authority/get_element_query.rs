use holo_hash::HeaderHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeader;

use super::WireDhtOp;

#[derive(Debug, Clone)]
pub struct GetElementOpsQuery(HeaderHash);

impl GetElementOpsQuery {
    pub fn new(hash: HeaderHash) -> Self {
        Self(hash)
    }
}

// TODO: Move this to holochain types.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WireElementOps {
    pub header: Option<WireDhtOp>,
    pub deletes: Vec<WireDhtOp>,
    pub updates: Vec<WireDhtOp>,
    pub entry: Option<Entry>,
}

impl WireElementOps {
    pub fn new() -> Self {
        Self {
            header: None,
            deletes: Vec::new(),
            updates: Vec::new(),
            entry: None,
        }
    }
}

impl Query for GetElementOpsQuery {
    type Data = WireDhtOp;
    type State = WireElementOps;
    type Output = Self::State;

    fn query(&self) -> &str {
        "
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
        DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:store_element, :delete, :update)
        AND
        DhtOp.basis_hash = :header_hash
        "
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

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        let f = |row: &Row| {
            let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
            let SignedHeader(header, signature) = header;
            let op_type = row.get(row.column_index("dht_type")?)?;
            let validation_status = row.get(row.column_index("status")?)?;
            Ok(WireDhtOp {
                validation_status,
                op_type,
                header,
                signature,
            })
        };
        Arc::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(WireElementOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Data) -> StateQueryResult<Self::State> {
        match &dht_op.op_type {
            DhtOpType::StoreElement => {
                if state.header.is_none() {
                    state.header = Some(dht_op);
                } else {
                    // TODO: This is weird there are multiple store elements ops for the same header??
                }
            }
            DhtOpType::RegisterDeletedBy => {
                state.deletes.push(dht_op);
            }
            DhtOpType::RegisterUpdatedElement => {
                state.updates.push(dht_op);
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, mut state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let entry_hash = state
            .header
            .as_ref()
            .and_then(|wire_op| wire_op.header.entry_hash());
        if let Some(entry_hash) = entry_hash {
            let entry = stores.get_entry(entry_hash)?;
            state.entry = entry;
        }
        Ok(state)
    }
}
