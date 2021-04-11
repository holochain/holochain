use holo_hash::*;
use holochain_sqlite::rusqlite::*;
use holochain_state::{prelude::*, query::row_blob_and_hash_to_header};
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct GetAgentActivityQuery {
    agent: AgentPubKey,
    filter: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
}

impl GetAgentActivityQuery {
    pub fn new(
        agent: AgentPubKey,
        filter: ChainQueryFilter,
        options: holochain_p2p::event::GetActivityOptions,
    ) -> Self {
        Self {
            agent,
            filter,
            options,
        }
    }
}

pub struct GetAgentActivityQueryState {
    valid: Vec<SignedHeaderHashed>,
    rejected: Vec<SignedHeaderHashed>,
}

impl Query for GetAgentActivityQuery {
    type Data = SignedHeaderHashed;
    type State = AgentActivityResponse<SignedHeaderHashed>;
    // NB: the current options also specify the ability to return only hashes.
    //     we either need a separate query for this, or we just post-process
    //     the full headers. Either way that option is ignored here.
    type Output = AgentActivityResponse<SignedHeaderHashed>;

    fn query(&self) -> String {
        let ChainQueryFilter {
            entry_type,
            header_type,
            sequence_range,
            include_entries: _,
            ..
        } = &self.filter;

        let entry_type_clause = entry_type
            .as_ref()
            .map(|_| "AND H.entry_type = :entry_type")
            .unwrap_or("");
        let header_type_clause = header_type
            .as_ref()
            .map(|_| "AND H.type = :header_type")
            .unwrap_or("");
        let range_clause = sequence_range
            .as_ref()
            .map(|_| "AND H.seq >= :range_start AND H.seq < :range_end")
            .unwrap_or("");
        format!(
            "
            SELECT H.blob, H.hash FROM Header AS H
            JOIN DhtOp as D
            ON D.header_hash = H.hash
            WHERE H.author = :author AND D.is_authored = 1
            {} {} {}
        ",
            entry_type_clause, header_type_clause, range_clause,
        )
    }

    fn params(&self) -> Vec<Params> {
        let mut params = (named_params! {
            ":author": self.agent,
            ":entry_type": self.filter.entry_type,
            ":header_type": self.filter.header_type,
        })
        .to_vec();

        if let Some(sequence_range) = &self.filter.sequence_range {
            params.extend(named_params! {
                ":range_start": sequence_range.start,
                ":range_end": sequence_range.end,
            })
        };
        params
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

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        todo!()
    }

    fn fold(&self, state: Self::State, sh: SignedHeaderHashed) -> StateQueryResult<Self::State> {
        todo!()
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        Ok(state)
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        Arc::new(row_blob_and_hash_to_header("blob", "hash"))
    }
}
