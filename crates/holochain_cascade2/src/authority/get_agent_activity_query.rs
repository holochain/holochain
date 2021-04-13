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

pub struct GetAgentActivityDeterministicQueryState {
    valid: Vec<SignedHeaderHashed>,
    rejected: Vec<SignedHeaderHashed>,
}

impl Query for GetAgentActivityDeterministicQuery {
    type Item = Judged<SignedHeaderHashed>;
    type State = AgentActivityResponse<SignedHeaderHashed>;
    // NB: the current options also specify the ability to return only hashes.
    //     we either need a separate query for this, or we just post-process
    //     the full headers. Either way that option is ignored here.
    type Output = AgentActivityResponse<SignedHeaderHashed>;

    fn query(&self) -> String {
        format!(
            "
            SELECT H.blob, H.hash FROM Header AS H
            JOIN DhtOp as D
            ON D.header_hash = H.hash
            WHERE H.author = :author
            AND D.validation_status IS NOT NULL  -- FIXME: ensure that it's actually valid
            AND (:entry_type IS NULL OR H.entry_type = :entry_type)
            AND (:header_type IS NULL OR H.type = :header_type)
            AND (:hash_low IS NULL OR H.seq >= (SELECT seq FROM Header WHERE hash = :hash_low))
            AND H.seq <= (SELECT seq FROM Header WHERE hash = :hash_high)
        "
        )
    }

    fn params(&self) -> Vec<Params> {
        (named_params! {
            ":author": self.agent,
            ":entry_type": self.filter.entry_type,
            ":header_type": self.filter.header_type,
            ":hash_low": self.filter.range.0,
            ":hash_high": self.filter.range.1,
        })
        .to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(Vec::new()),
            rejected_activity: ChainItems::Full(Vec::new()),
            status: ChainStatus::Empty,
            highest_observed: None,
        })
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        todo!()
    }

    fn fold(&self, state: Self::State, data: Self::Item) -> StateQueryResult<Self::State> {
        todo!()
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state)
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        let f = row_blob_and_hash_to_header("blob", "hash");
        // Data is valid because iI'm not sure why?
        Arc::new(move |row| Ok(Judged::valid(f(row)?)))
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
            let mut top_header = None;
            for _i in 0..10 {
                let mut header_create = fixt!(Create);
                header_create.entry_type = entry_type_1.clone();
                header_create.author = agents[a].clone();
                top_header = Some(header_create.clone());
                let op_create = DhtOp::StoreEntry(
                    fixt!(Signature),
                    header_create.into(),
                    Box::new(fixt!(Entry)),
                );
                let op = DhtOpHashed::from_content_sync(op_create);
                dbg!(HeaderHash::with_data_sync(&op.header()));
                fill_db(&env, op);
            }
            top_hashes.push(HeaderHash::with_data_sync(&Header::from(
                top_header.unwrap(),
            )));
        }

        dbg!(&top_hashes);

        let filter = AgentActivityFilterDeterministic {
            range: (None, top_hashes[2].clone()),
            entry_type: Some(entry_type_1),
            header_type: None,
            include_entries: false,
        };
        let options = GetActivityOptions::default();
        let results = handle_get_agent_activity(
            env.clone().into(),
            agents[2].clone(),
            filter.clone(),
            options,
        )
        .unwrap();

        dbg!(&results);

        matches::assert_matches!(results.valid_activity, ChainItems::Full(items) if items.len() == 10)
    }
}
