use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[cfg(test)]
mod test;

#[derive(Debug, Clone)]
pub struct GetLiveEntryQuery(EntryHash);

impl GetLiveEntryQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

impl Query for GetLiveEntryQuery {
    type Data = SignedHeaderHashed;
    type ValidatedData = ValStatusOf<Self::Data>;
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
        AND (DhtOp.when_integrated IS NOT NULL OR DhtOp.is_authored = 1)
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
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::ValidatedData>> {
        let f = row_blob_to_header("header_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(ValStatusOf::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        let entry_filter = self.0.clone();
        let f = move |header: &Self::Data| match header.header() {
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

    fn fold(
        &self,
        mut state: Self::State,
        data: Self::ValidatedData,
    ) -> StateQueryResult<Self::State> {
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
                    if !state.deletes.contains(&hash) {
                        state.creates.insert(hash, shh);
                    }
                // TODO: This is where update chains will be followed
                // when we add that functionality.
                } else if update.entry_hash == self.0 {
                    if !state.deletes.contains(&hash) {
                        state.creates.insert(hash, shh);
                    }
                } else if update.original_entry_address == self.0 {
                    // TODO: This is where update chains will be followed
                    // when we add that functionality.
                }
            }
            Header::Delete(delete) => {
                state.creates.remove(&delete.deletes_address);
                state.deletes.insert(delete.deletes_address.clone());
            }
            _ => panic!("TODO: Turn this into an error"),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        // Choose an arbitrary header
        let header = state.creates.into_iter().map(|(_, v)| v).next();
        match header {
            Some(header) => {
                // TODO: Handle error where header doesn't have entry hash.
                let entry_hash = header.header().entry_hash().unwrap();
                let entry = stores
                    .get_entry(&entry_hash)?
                    .expect("TODO: Handle case where entry wasn't found but we had headers");
                Ok(Some(Element::new(header, Some(entry))))
            }
            None => Ok(None),
        }
    }
}
