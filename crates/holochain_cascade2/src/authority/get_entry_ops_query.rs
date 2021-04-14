use holo_hash::EntryHash;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::Row;
use holochain_state::query::StateQueryError;
use holochain_state::query::{prelude::*, QueryData};
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::DhtOpError;
use holochain_types::prelude::HasValidationStatus;
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
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct WireEntryOps {
    pub creates: Vec<WireDhtOp>,
    pub deletes: Vec<WireDhtOp>,
    pub updates: Vec<WireDhtOp>,
    pub entry: Option<Entry>,
}

impl WireEntryOps {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WireDhtOp {
    pub validation_status: Option<ValidationStatus>,
    pub op_type: DhtOpType,
    pub header: Header,
    pub signature: Signature,
}

impl HasValidationStatus for WireDhtOp {
    type Data = Self;

    fn validation_status(&self) -> Option<ValidationStatus> {
        self.validation_status
    }

    fn data(&self) -> &Self {
        self
    }
}

impl Query for GetEntryOpsQuery {
    type Item = WireDhtOp;
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
            let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?)?;
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

    fn fold(
        &self,
        mut state: Self::State,
        dht_op: QueryData<Self>,
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
            _ => return Err(StateQueryError::UnexpectedOp(dht_op.op_type)),
        }
        Ok(state)
    }

    fn render<S>(&self, mut state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let wire_op = state.creates.first();
        let entry_hash = match wire_op {
            Some(wire_op) => Some(
                wire_op
                    .header
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(wire_op.header.clone()))?,
            ),
            None => None,
        };
        if let Some(entry_hash) = entry_hash {
            let entry = stores.get_entry(entry_hash)?;
            state.entry = entry;
        }
        Ok(state)
    }
}
