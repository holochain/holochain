use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::Judged;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetEntryDetailsQuery(EntryHash);

impl GetEntryDetailsQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

pub struct State {
    headers: HashSet<SignedHeaderHashed>,
    rejected_headers: HashSet<SignedHeaderHashed>,
    deletes: HashMap<HeaderHash, SignedHeaderHashed>,
    updates: HashSet<SignedHeaderHashed>,
}

impl Query for GetEntryDetailsQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = State;
    type Output = Option<EntryDetails>;

    fn query(&self) -> String {
        "
        SELECT Header.blob AS header_blob, DhtOp.validation_status AS status
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :entry_hash
        AND (DhtOp.when_integrated IS NOT NULL OR DhtOp.is_authored = 1)
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreEntry,
            ":delete_type": DhtOpType::RegisterDeletedEntryHeader,
            ":update_type": DhtOpType::RegisterUpdatedContent,
            ":entry_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = |row: &Row| {
            let header = from_blob::<SignedHeader>(row.get(row.column_index("header_blob")?)?);
            let SignedHeader(header, signature) = header;
            let header = HeaderHashed::from_content_sync(header);
            let shh = SignedHeaderHashed::with_presigned(header, signature);
            let status = row.get(row.column_index("status")?)?;
            let r = Judged { data: shh, status };
            Ok(r)
        };
        Arc::new(f)
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let entry_filter = self.0.clone();
        let f = move |header: &QueryData<Self>| {
            let header = &header;
            match header.header() {
                Header::Create(Create { entry_hash, .. })
                | Header::Update(Update { entry_hash, .. })
                    if *entry_hash == entry_filter =>
                {
                    true
                }
                Header::Update(Update {
                    original_entry_address,
                    ..
                }) => *original_entry_address == entry_filter,
                Header::Delete(Delete {
                    deletes_entry_address,
                    ..
                }) => *deletes_entry_address == entry_filter,
                _ => false,
            }
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(State {
            headers: Default::default(),
            rejected_headers: Default::default(),
            deletes: Default::default(),
            updates: Default::default(),
        })
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let Judged {
            data: shh,
            status: validation_status,
        } = data;
        let add_header = |state: &mut State, shh| match validation_status {
            Some(ValidationStatus::Valid) => {
                state.headers.insert(shh);
            }
            Some(ValidationStatus::Rejected) => {
                state.rejected_headers.insert(shh);
            }
            _ => (),
        };
        match shh.header() {
            Header::Create(_) => add_header(&mut state, shh),
            Header::Update(update) => {
                if update.original_entry_address == self.0 && update.entry_hash == self.0 {
                    state.updates.insert(shh.clone());
                    add_header(&mut state, shh);
                } else if update.entry_hash == self.0 {
                    add_header(&mut state, shh);
                } else if update.original_entry_address == self.0 {
                    state.updates.insert(shh.clone());
                }
            }
            Header::Delete(delete) => {
                let hash = delete.deletes_address.clone();
                state.deletes.insert(hash, shh.clone());
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        // Choose an arbitrary header.
        // TODO: Is it sound to us a rejected header here?
        let header = state
            .headers
            .iter()
            .chain(state.rejected_headers.iter())
            .next();
        match header {
            Some(header) => {
                // TODO: Handle error where header doesn't have entry hash.
                let entry_hash = header.header().entry_hash().unwrap();
                let entry = stores
                    .get_entry(&entry_hash)?
                    .expect("TODO: Handle case where entry wasn't found but we had headers");
                let entry_dht_status = compute_entry_status(&state);
                let details = EntryDetails {
                    entry,
                    headers: state.headers.into_iter().collect(),
                    rejected_headers: state.rejected_headers.into_iter().collect(),
                    deletes: state.deletes.into_iter().map(|(_, v)| v).collect(),
                    updates: state.updates.into_iter().collect(),
                    entry_dht_status: entry_dht_status,
                };
                Ok(Some(details))
            }
            None => Ok(None),
        }
    }
}

fn compute_entry_status(state: &State) -> EntryDhtStatus {
    let live_headers = state
        .headers
        .iter()
        .filter(|h| !state.deletes.contains_key(h.header_address()))
        .count();
    if live_headers > 0 {
        EntryDhtStatus::Live
    } else {
        EntryDhtStatus::Dead
    }
}
