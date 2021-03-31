use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetQuery(EntryHash);

impl GetQuery {
    pub fn new(hash: EntryHash) -> Self {
        Self(hash)
    }
}

impl Query for GetQuery {
    type State = Maps<SignedHeaderHashed>;
    type Output = Option<Element>;

    fn create_query(&self) -> &str {
        "
            SELECT Header.blob AS header_blob FROM DhtOp
            JOIN Header On DhtOp.header_hash = Header.hash
            WHERE DhtOp.type = :store_entry
            AND
            DhtOp.basis_hash = :entry_hash
        "
    }

    fn delete_query(&self) -> &str {
        "
        SELECT Header.blob AS header_blob FROM DhtOp
        JOIN Header On DhtOp.header_hash = Header.hash
        WHERE DhtOp.type = :delete
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

    fn init_fold(&self) -> Result<Self::State, PlaceHolderError> {
        Ok(Maps::new())
    }

    fn as_filter(&self) -> Box<dyn Fn(&Header) -> bool> {
        let entry_filter = self.0.clone();
        let f = move |header: &Header| match header {
            Header::Create(Create { entry_hash, .. }) => *entry_hash == entry_filter,
            Header::Delete(Delete {
                deletes_entry_address,
                ..
            }) => *deletes_entry_address == entry_filter,
            _ => false,
        };
        Box::new(f)
    }

    fn fold(
        &mut self,
        mut state: Self::State,
        shh: SignedHeaderHashed,
    ) -> Result<Self::State, PlaceHolderError> {
        let hash = shh.as_hash().clone();
        match shh.header() {
            Header::Create(_) => {
                if !state.deletes.contains(&hash) {
                    state.creates.insert(hash, shh);
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

    fn render(
        &mut self,
        state: Self::State,
        txns: &Transactions<'_, '_>,
    ) -> Result<Self::Output, PlaceHolderError> {
        // Choose an arbitrary header
        let header = state.creates.into_iter().map(|(_, v)| v).next();
        match header {
            Some(header) => {
                for txn in txns.into_iter() {
                    let entry_hash = header.header().entry_hash().unwrap();
                    let entry = txn.query_row_named(
                        "
                    SELECT Entry.blob AS entry_blob FROM Entry
                    WHERE hash = :entry_hash
                    ",
                        named_params! {
                            ":entry_hash": entry_hash.clone(),
                        },
                        |row| {
                            Ok(from_blob::<Entry>(
                                row.get(row.column_index("entry_blob")?)?,
                            ))
                        },
                    );
                    if let Err(holochain_sqlite::rusqlite::Error::QueryReturnedNoRows) = &entry {
                        continue;
                    } else {
                        // TODO: Handle this error.
                        let entry = entry.unwrap();
                        return Ok(Some(Element::new(header, Some(entry))));
                    }
                }
                panic!("TODO: Handle case where entry wasn't found but we had headers")
            }
            None => Ok(None),
        }
    }
}
