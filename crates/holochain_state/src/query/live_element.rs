use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[cfg(test)]
mod test;

#[derive(Debug, Clone)]
pub struct GetLiveElementQuery(HeaderHash, Option<Arc<AgentPubKey>>);

impl GetLiveElementQuery {
    pub fn new(hash: HeaderHash) -> Self {
        Self(hash, None)
    }
}

impl Query for GetLiveElementQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = (Option<SignedHeaderHashed>, HashSet<HeaderHash>);
    type Output = Option<Element>;

    fn query(&self) -> String {
        "
        SELECT Header.blob AS header_blob
        FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type IN (:create_type, :delete_type, :update_type)
        AND DhtOp.basis_hash = :header_hash
        AND DhtOp.validation_status = :status
        AND DhtOp.when_integrated IS NOT NULL
        "
        .into()
    }
    fn params(&self) -> Vec<Params> {
        let params = named_params! {
            ":create_type": DhtOpType::StoreElement,
            ":delete_type": DhtOpType::RegisterDeletedBy,
            ":update_type": DhtOpType::RegisterUpdatedElement,
            ":status": ValidationStatus::Valid,
            ":header_hash": self.0,
        };
        params.to_vec()
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_to_header("header_blob");
        // Data is valid because it is filtered in the sql query.
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        let header_filter = self.0.clone();
        let f = move |header: &QueryData<Self>| {
            if *header.header_address() == header_filter {
                true
            } else if let Header::Delete(Delete {
                deletes_address, ..
            }) = header.header()
            {
                *deletes_address == header_filter
            } else {
                false
            }
        };
        Box::new(f)
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok((None, HashSet::new()))
    }

    fn fold(&self, mut state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        let shh = data.data;
        let hash = shh.as_hash();
        if *hash == self.0 && state.0.is_none() {
            if !state.1.contains(hash) {
                state.0 = Some(shh);
            }
        } else if let Header::Delete(delete) = shh.header() {
            let header = state.0.take();
            if let Some(h) = header {
                if *h.as_hash() != delete.deletes_address {
                    state.0 = Some(h);
                }
            }
            state.1.insert(delete.deletes_address.clone());
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        match state.0 {
            Some(header) => {
                let mut entry = None;
                if let Some(entry_hash) = header.header().entry_hash() {
                    let author = self
                        .1
                        .as_ref()
                        .map(|a| a.as_ref())
                        .filter(|a| *a == header.header().author());
                    entry = stores.get_public_or_authored_entry(entry_hash, author)?;
                }
                Ok(Some(Element::new(header, entry)))
            }
            None => Ok(None),
        }
    }
}

impl PrivateDataQuery for GetLiveElementQuery {
    type Hash = HeaderHash;

    fn with_private_data_access(hash: Self::Hash, author: Arc<AgentPubKey>) -> Self {
        Self(hash, Some(author))
    }

    fn without_private_data_access(hash: Self::Hash) -> Self {
        Self::new(hash)
    }
}
