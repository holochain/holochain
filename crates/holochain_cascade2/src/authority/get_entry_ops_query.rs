use holo_hash::EntryHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::Signature;
use holochain_zome_types::SignedHeader;
use holochain_zome_types::ValidationStatus;

#[derive(Debug, Clone)]
pub struct GetEntryOpsQuery(EntryHash);

impl GetEntryOpsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

// TODO: Move this to holochain types.
// TODO: This currently looks the same as
// [`WireElementOps`] but there are more things
// we can condense on entry ops due to sharing the
// same entry hash.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WireEntryOps {
    pub creates: Vec<WireDhtOp>,
    pub deletes: Vec<WireDhtOp>,
    pub updates: Vec<WireDhtOp>,
    pub entry: Option<Entry>,
}

impl WireEntryOps {
    pub fn new() -> Self {
        Self {
            creates: Vec::new(),
            deletes: Vec::new(),
            updates: Vec::new(),
            entry: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WireDhtOp {
    pub validation_status: Option<ValidationStatus>,
    pub op_type: DhtOpType,
    pub header: Header,
    pub signature: Signature,
}

impl Query for GetEntryOpsQuery {
    type Data = WireDhtOp;
    type State = WireEntryOps;
    type Output = Self::State;

    fn query(&self) -> String {
        "
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type,
        DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:store_entry, :delete, :update)
        AND
        DhtOp.basis_hash = :entry_hash
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
        Ok(WireEntryOps::new())
    }

    fn fold(&self, mut state: Self::State, dht_op: Self::Data) -> StateQueryResult<Self::State> {
        match &dht_op.op_type {
            DhtOpType::StoreEntry => {
                state.creates.push(dht_op);
            }
            DhtOpType::RegisterDeletedEntryHeader => {
                state.deletes.push(dht_op);
            }
            DhtOpType::RegisterUpdatedContent => {
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
        // TODO: Handle error where header is missing entry hash.
        let entry_hash = state
            .creates
            .first()
            .map(|wire_op| wire_op.header.entry_hash().unwrap());
        if let Some(entry_hash) = entry_hash {
            let entry = stores.get_entry(entry_hash)?;
            state.entry = entry;
        }
        Ok(state)
    }
}
