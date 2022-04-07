//! Query for `deterministic_get_agent_activity`, designed for use in
//! validation callbacks.
//!
//! This is a deterministic version of `get_agent_activity`, designed such that
//! there can only be one possible valid result which satisfies the query
//! criteria, so if you get back a result, you can verify that it is correct
//! and safely use it in your own validation. If you don't get a value back,
//! you cannot proceed with validation.
//!
//! - The agent authority will fully validate Headers, so it's OK to pass the
//!   full headers to Wasm
//! - Must return a contiguous range of Headers so that the requestor can
//!   ensure that the data is valid (TODO we're skipping the actual validation
//!   on the requestor side for now).

use holo_hash::*;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::*;
use holochain_state::{
    prelude::*,
    query::{row_blob_and_hash_to_header, QueryData},
};
use holochain_types::prelude::*;
use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeterministicGetAgentActivityQuery {
    agent: AgentPubKey,
    filter: DeterministicGetAgentActivityFilter,
    options: GetActivityOptions,
}

impl DeterministicGetAgentActivityQuery {
    pub fn new(
        agent: AgentPubKey,
        filter: DeterministicGetAgentActivityFilter,
        options: GetActivityOptions,
    ) -> Self {
        Self {
            agent,
            filter,
            options,
        }
    }
}

#[derive(Debug)]
pub struct DeterministicGetAgentActivityQueryState {
    chain: Vec<Judged<SignedHeader>>,
    prev_header: Option<HeaderHash>,
}

impl Query for DeterministicGetAgentActivityQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = DeterministicGetAgentActivityQueryState;
    type Output = DeterministicGetAgentActivityResponse;

    fn query(&self) -> String {
        "
            SELECT H.blob, H.hash, D.validation_status FROM Header AS H
            JOIN DhtOp as D
            ON D.header_hash = H.hash
            WHERE H.author = :author
            AND D.type = :op_type
            AND D.validation_status IS NOT NULL
            AND D.when_integrated IS NOT NULL
            AND (:hash_low IS NULL OR H.seq >= (SELECT seq FROM Header WHERE hash = :hash_low))
            AND H.seq <= (SELECT seq FROM Header WHERE hash = :hash_high)
            ORDER BY H.seq DESC
        "
        .to_string()
    }

    fn params(&self) -> Vec<holochain_state::query::Params> {
        (named_params! {
            ":author": self.agent,
            ":hash_low": self.filter.range.0,
            ":hash_high": self.filter.range.1,
            ":op_type": DhtOpType::RegisterAgentActivity,
        })
        .to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(DeterministicGetAgentActivityQueryState {
            chain: Vec::new(),
            prev_header: Some(self.filter.range.1.clone()),
        })
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        todo!()
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        let (shh, status) = item.into();
        let SignedHeaderHashed {
            hashed:
                HeaderHashed {
                    content: header,
                    hash,
                },
            signature,
        } = shh;
        let sh = SignedHeader(header, signature);
        // By tracking the prev_header of the last header we added to the chain,
        // we can filter out branches. If we performed branch detection in this
        // query, it would not be deterministic.
        //
        // TODO: ensure that this still works with the scratch, and that we
        // never have to run this query including the Cache. That is, if we join
        // results from multiple Stores, the ordering of header_seq will be
        // discontinuous, and we will have to collect into a sorted list before
        // doing this fold.
        if Some(hash) == state.prev_header {
            state.prev_header = sh.header().prev_header().cloned();
            state.chain.push((sh, status).into());
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(DeterministicGetAgentActivityResponse::new(state.chain))
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_and_hash_to_header("blob", "hash");
        Arc::new(move |row| {
            let validation_status: ValidationStatus = row.get("validation_status")?;
            Ok(Judged::new(f(row)?, validation_status))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::fill_db;
    use ::fixt::prelude::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_activity_query() {
        observability::test_run().ok();
        let test_db = test_dht_db();
        let db = test_db.to_db();
        let entry_type_1 = fixt!(EntryType);
        let agents = [fixt!(AgentPubKey), fixt!(AgentPubKey), fixt!(AgentPubKey)];
        let mut chains = vec![];

        for a in 0..3 {
            let mut chain: Vec<HeaderHash> = Vec::new();
            for seq in 0..10 {
                let header: Header = if let Some(top) = chain.last() {
                    let mut header = fixt!(Create);
                    header.entry_type = entry_type_1.clone();
                    header.author = agents[a].clone();
                    header.prev_header = top.clone();
                    header.header_seq = seq;
                    let entry = Entry::App(fixt!(AppEntryBytes));
                    header.entry_hash = EntryHash::with_data_sync(&entry);
                    header.into()
                } else {
                    let mut header = fixt!(Dna);
                    header.author = agents[a].clone();
                    header.into()
                };
                chain.push(HeaderHash::with_data_sync(&header));
                let op = DhtOp::RegisterAgentActivity(fixt!(Signature), header.into());
                let op = DhtOpHashed::from_content_sync(op);
                fill_db(&db, op);
            }
            chains.push(chain);
        }

        let filter_full = DeterministicGetAgentActivityFilter {
            range: (None, chains[2].last().unwrap().clone()),
            entry_type: Some(entry_type_1.clone()),
            header_type: None,
            include_entries: false,
        };

        let filter_partial = DeterministicGetAgentActivityFilter {
            range: (Some(chains[2][4].clone()), chains[2][8].clone()),
            entry_type: Some(entry_type_1),
            header_type: None,
            include_entries: false,
        };
        let options = GetActivityOptions::default();

        let results_full = crate::authority::handle_get_agent_activity_deterministic(
            db.clone().into(),
            agents[2].clone(),
            filter_full,
            options.clone(),
        )
        .await
        .unwrap();

        let results_partial = crate::authority::handle_get_agent_activity_deterministic(
            db.clone().into(),
            agents[2].clone(),
            filter_partial,
            options,
        )
        .await
        .unwrap();

        assert_eq!(results_full.chain.len(), 10);
        assert_eq!(results_partial.chain.len(), 5);
    }
}
