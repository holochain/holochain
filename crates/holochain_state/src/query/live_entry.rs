use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::DhtOpError;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[cfg(test)]
mod test;

#[derive(Debug, Clone)]
pub struct GetLiveEntryQuery(EntryHash, Option<Arc<AgentPubKey>>);

impl GetLiveEntryQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash, None)
    }
}

impl Query for GetLiveEntryQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = Maps<SignedHeaderHashed>;
    type Output = Option<Element>;

    fn query(&self) -> String {
        "
        SELECT Header.blob AS header_blob
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :entry_hash
        AND DhtOp.validation_status = :status
        AND DhtOp.when_integrated IS NOT NULL
        AND (Header.private_entry = 0 OR Header.private_entry IS NULL OR Header.author = :author)
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreEntry,
            ":delete_type": DhtOpType::RegisterDeletedEntryHeader,
            ":update_type": DhtOpType::RegisterUpdatedContent,
            ":status": ValidationStatus::Valid,
            ":entry_hash": self.0,
            ":author": self.1,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_to_header("header_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let entry_filter = self.0.clone();
        let f = move |header: &QueryData<Self>| match header.header() {
            Header::Create(Create { entry_hash, .. })
            | Header::Update(Update { entry_hash, .. }) => *entry_hash == entry_filter,
            Header::Delete(Delete {
                deletes_entry_address,
                ..
            }) => *deletes_entry_address == entry_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Maps::new())
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let hash = shh.as_hash().clone();
        match shh.header() {
            Header::Create(_) => {
                if !state.deletes.contains(&hash) {
                    state.creates.insert(hash, shh);
                }
            }
            Header::Update(update) => {
                if update.original_entry_address == self.0 && update.entry_hash == self.0 {
                    follow_update_chain(&state, &shh);
                    if !state.deletes.contains(&hash) {
                        state.creates.insert(hash, shh);
                    }
                } else if update.entry_hash == self.0 {
                    if !state.deletes.contains(&hash) {
                        state.creates.insert(hash, shh);
                    }
                } else if update.original_entry_address == self.0 {
                    follow_update_chain(&state, &shh);
                }
            }
            Header::Delete(delete) => {
                state.creates.remove(&delete.deletes_address);
                state.deletes.insert(delete.deletes_address.clone());
            }
            _ => {
                return Err(StateQueryError::UnexpectedHeader(
                    shh.header().header_type(),
                ))
            }
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        // If we have author authority then find a header from this author.
        let authored_header = self.1.as_ref().map(|a| a.as_ref()).and_then(|a| {
            state
                .creates
                .iter()
                .map(|(_, v)| v)
                .find(|h| *h.header().author() == *a)
                .cloned()
        });
        let is_authored = authored_header.is_some();
        // If there is no authored header, choose an arbitrary header.
        let header = authored_header.or_else(|| {
            // The line below was added when migrating to rust edition 2021, per
            // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
            let _ = &state;
            state.creates.into_iter().map(|(_, v)| v).next()
        });
        match header {
            Some(header) => {
                let entry_hash = header
                    .header()
                    .entry_hash()
                    .ok_or_else(|| DhtOpError::HeaderWithoutEntry(header.header().clone()))?;
                // If this header is authored then we can get an authored entry.
                let author = is_authored.then(|| header.header().author());
                let element = stores
                    .get_public_or_authored_entry(entry_hash, author)?
                    .map(|entry| Element::new(header, Some(entry)));
                Ok(element)
            }
            None => Ok(None),
        }
    }
}

fn follow_update_chain(_state: &Maps<SignedHeaderHashed>, _shh: &SignedHeaderHashed) {
    // TODO: This is where update chains will be followed
    // when we add that functionality.
}

impl PrivateDataQuery for GetLiveEntryQuery {
    type Hash = EntryHash;

    fn with_private_data_access(hash: Self::Hash, author: Arc<AgentPubKey>) -> Self {
        Self(hash, Some(author))
    }

    fn without_private_data_access(hash: Self::Hash) -> Self {
        Self::new(hash)
    }
}
