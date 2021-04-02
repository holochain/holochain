use holo_hash::EntryHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::{prelude::*, StoresIter};
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::Entry;
use holochain_zome_types::Header;
use holochain_zome_types::Signature;
use holochain_zome_types::SignedHeader;

#[derive(Debug, Clone)]
pub struct GetEntryOpsQuery(EntryHash);

impl GetEntryOpsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

// TODO: Move this to holochain types.
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
    pub op_type: DhtOpType,
    pub header: Header,
    pub signature: Signature,
}

impl Query for GetEntryOpsQuery {
    type Data = WireDhtOp;
    type State = WireEntryOps;
    type Output = Self::State;

    fn create_query(&self) -> &str {
        "
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :store_entry
        AND
        DhtOp.basis_hash = :entry_hash
        "
    }

    fn delete_query(&self) -> &str {
        "
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
        AND
        DhtOp.basis_hash = :entry_hash
        "
    }

    fn update_query(&self) -> &str {
        "
        SELECT Header.blob AS header_blob, DhtOp.type AS dht_type
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :update
        AND
        DhtOp.basis_hash = :entry_hash
        "
    }

    fn create_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":store_entry": DhtOpType::StoreEntry,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn delete_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":delete": DhtOpType::RegisterDeletedEntryHeader,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn update_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":update": DhtOpType::RegisterUpdatedContent,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(WireEntryOps::new())
    }

    fn fold(
        &mut self,
        mut state: Self::State,
        dht_op: Self::Data,
    ) -> StateQueryResult<Self::State> {
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

    fn render<S>(&mut self, mut state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Stores<Self>,
        S::O: StoresIter<Self::Data>,
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

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        let f = |row: &Row| {
            let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
            let SignedHeader(header, signature) = header;
            let op_type = row.get(row.column_index("dht_type")?)?;
            Ok(WireDhtOp {
                op_type,
                header,
                signature,
            })
        };
        Arc::new(f)
    }
}
