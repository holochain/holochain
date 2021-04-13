use holo_hash::*;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::*;
use holochain_state::{
    prelude::*,
    query::{row_blob_and_hash_to_header, QueryData},
};
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetAgentActivityDeterministicQuery {
    agent: AgentPubKey,
    filter: AgentActivityFilterDeterministic,
    options: GetActivityOptions,
}

impl GetAgentActivityDeterministicQuery {
    pub fn new(
        agent: AgentPubKey,
        filter: AgentActivityFilterDeterministic,
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
pub struct GetAgentActivityDeterministicQueryState {
    chain: Vec<Judged<SignedHeader>>,
    prev_header: Option<HeaderHash>,
}

impl Query for GetAgentActivityDeterministicQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = GetAgentActivityDeterministicQueryState;
    type Output = AgentActivityResponseDeterministic;

    fn query(&self) -> String {
        format!(
            "
            SELECT H.blob, H.hash, D.validation_status FROM Header AS H
            JOIN DhtOp as D
            ON D.header_hash = H.hash
            WHERE H.author = :author
            AND D.type = :op_type
            AND D.validation_status IS NOT NULL
            AND D.when_integrated IS NOT NULL
            AND (:hash_low IS NULL OR H.seq >= (SELECT seq FROM Header WHERE hash = :hash_low))
            -- AND H.seq <= (SELECT seq FROM Header WHERE hash = :hash_high)
            ORDER BY H.seq DESC
        "
        )
    }

    fn params(&self) -> Vec<Params> {
        (named_params! {
            ":author": self.agent,
            ":hash_low": self.filter.range.0,
            // ":hash_high": self.filter.range.1,
            ":op_type": DhtOpType::RegisterAgentActivity,
        })
        .to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(GetAgentActivityDeterministicQueryState {
            chain: Vec::new(),
            prev_header: Some(self.filter.range.1.clone()),
        })
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        todo!()
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        dbg!((state.chain.len(), &state.prev_header), &item.data.header_hashed());
        let (shh, status) = item.into();
        let (header, hash) = shh.into_inner();
        // By tracking the prev_header of the last header we added to the chain,
        // we can filter out branches. If we performed branch detection in this
        // query, it would not be deterministic.
        if Some(hash) == state.prev_header {
            state.prev_header = header.header().prev_header().cloned();
            state.chain.push((header, status).into());
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(AgentActivityResponseDeterministic::new(state.chain))
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
        let test_env = test_cell_env();
        let env = test_env.env();
        let entry_type_1 = fixt!(EntryType);
        let agents = [fixt!(AgentPubKey), fixt!(AgentPubKey), fixt!(AgentPubKey)];
        let mut top_hashes = vec![];

        for a in 0..3 {
            let mut top_hash: Option<HeaderHash> = None;
            for seq in 0..10 {
                let header: Header = if let Some(top_hash) = top_hash {
                    let mut header = fixt!(Create);
                    header.entry_type = entry_type_1.clone();
                    header.author = agents[a].clone();
                    header.prev_header = top_hash.clone();
                    header.header_seq = seq;
                    let entry = Entry::App(fixt!(AppEntryBytes));
                    header.entry_hash = EntryHash::with_data_sync(&entry);
                    header.into()
                } else {
                    let mut header = fixt!(Dna);
                    header.author = agents[a].clone();
                    header.into()
                };
                top_hash = Some(HeaderHash::with_data_sync(&header));
                let op = DhtOp::RegisterAgentActivity(fixt!(Signature), header.into());
                let op = DhtOpHashed::from_content_sync(op);
                fill_db(&env, op);
            }
            top_hashes.push(top_hash.unwrap());
        }

        let filter_full = AgentActivityFilterDeterministic {
            range: (None, top_hashes[2].clone()),
            entry_type: Some(entry_type_1.clone()),
            header_type: None,
            include_entries: false,
        };

        // let filter_partial = AgentActivityFilterDeterministic {
        //     range: (None, top_hashes[2].clone()),
        //     entry_type: Some(entry_type_1),
        //     header_type: None,
        //     include_entries: false,
        // };
        let options = GetActivityOptions::default();
        let results = handle_get_agent_activity(
            env.clone().into(),
            agents[2].clone(),
            filter_full.clone(),
            options,
        )
        .unwrap();

        assert_eq!(results.chain.len(), 10);
    }
}
