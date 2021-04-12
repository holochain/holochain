use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::ValStatusOf;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetElementDetailsQuery(HeaderHash);

impl GetElementDetailsQuery {
    pub fn new(hash: HeaderHash) -> Self {
        Self(hash)
    }
}

pub struct State {
    header: Option<SignedHeaderHashed>,
    rejected_header: Option<SignedHeaderHashed>,
    deletes: HashSet<SignedHeaderHashed>,
    updates: HashSet<SignedHeaderHashed>,
}

impl Query for GetElementDetailsQuery {
    type Data = ValStatusOf<SignedHeaderHashed>;
    type State = State;
    type Output = Option<ElementDetails>;

    fn query(&self) -> String {
        "
        SELECT Header.blob AS header_blob, DhtOp.validation_status AS status
        SELECT Header.blob AS header_blob
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :header_hash
        AND (DhtOp.when_integrated IS NOT NULL OR DhtOp.is_authored = 1)
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreEntry,
            ":delete_type": DhtOpType::RegisterDeletedEntryHeader,
            ":update_type": DhtOpType::RegisterUpdatedContent,
            ":header_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        let f = |row: &Row| {
            let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
            let SignedHeader(header, signature) = header;
            let header = HeaderHashed::from_content_sync(header);
            let shh = SignedHeaderHashed::with_presigned(header, signature);
            let status = row.get(row.column_index("status")?)?;
            let r = ValStatusOf { data: shh, status };
            Ok(r)
        };
        Arc::new(f)
    }

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        let header_filter = self.0.clone();
        let f = move |header: &Self::Data| {
            let header = &header.data;
            if *header.header_address() == header_filter {
                true
            } else {
                match header.header() {
                    Header::Delete(Delete {
                        deletes_address, ..
                    }) => *deletes_address == header_filter,
                    Header::Update(Update {
                        original_header_address,
                        ..
                    }) => *original_header_address == header_filter,
                    _ => false,
                }
            }
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(State {
            header: Default::default(),
            rejected_header: Default::default(),
            deletes: Default::default(),
            updates: Default::default(),
        })
    }

    fn fold(&self, mut state: Self::State, data: Self::Data) -> StateQueryResult<Self::State> {
        let ValStatusOf {
            data: shh,
            status: validation_status,
        } = data;
        let add_header = |state: &mut State, shh| {
            if state.header.is_none() && state.rejected_header.is_none() {
                match validation_status {
                    Some(ValidationStatus::Valid) => {
                        state.header = Some(shh);
                    }
                    Some(ValidationStatus::Rejected) => {
                        state.rejected_header = Some(shh);
                    }
                    _ => (),
                }
            }
        };
        match shh.header() {
            Header::Create(_) => add_header(&mut state, shh),
            Header::Update(update) => {
                if update.original_header_address == self.0 && *shh.as_hash() == self.0 {
                    state.updates.insert(shh.clone());
                    add_header(&mut state, shh);
                } else if *shh.as_hash() == self.0 {
                    add_header(&mut state, shh);
                } else if update.original_header_address == self.0 {
                    state.updates.insert(shh.clone());
                }
            }
            Header::Delete(_) => {
                state.deletes.insert(shh);
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let State {
            header,
            rejected_header,
            deletes,
            updates,
        } = state;

        let (header, validation_status) = match (header, rejected_header) {
            (None, None) => return Ok(None),
            (None, Some(h)) => (h, ValidationStatus::Rejected),
            (Some(h), None) => (h, ValidationStatus::Valid),
            (Some(_), Some(h)) => {
                // TODO: this is a conflict between multiple sources and
                // needs to be handled.
                (h, ValidationStatus::Rejected)
            }
        };

        let mut entry = None;
        if let Some(entry_hash) = header.header().entry_hash() {
            entry = stores.get_entry(entry_hash)?;
        }
        let element = Element::new(header, entry);
        let details = ElementDetails {
            element,
            validation_status,
            deletes: deletes.into_iter().collect(),
            updates: updates.into_iter().collect(),
        };
        Ok(Some(details))
    }
}
