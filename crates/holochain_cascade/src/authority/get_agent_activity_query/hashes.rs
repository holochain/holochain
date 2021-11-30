use holo_hash::*;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::*;
use holochain_state::{prelude::*, query::QueryData};
use holochain_zome_types::Judged;
use holochain_zome_types::*;
use std::fmt::Debug;
use std::sync::Arc;

use crate::authority::*;

#[derive(Debug, Clone)]
pub struct GetAgentActivityQuery {
    agent: AgentPubKey,
    filter: ChainQueryFilter,
    options: GetActivityOptions,
}

impl GetAgentActivityQuery {
    pub fn new(agent: AgentPubKey, filter: ChainQueryFilter, options: GetActivityOptions) -> Self {
        Self {
            agent,
            filter,
            options,
        }
    }
}

#[derive(Debug, Default)]
pub struct State {
    valid: Vec<HeaderHashed>,
    rejected: Vec<HeaderHashed>,
    pending: Vec<HeaderHashed>,
    status: Option<ChainStatus>,
}

#[derive(Debug)]
pub enum Item {
    Integrated(HeaderHashed),
    Pending(HeaderHashed),
}

impl Query for GetAgentActivityQuery {
    type Item = Judged<Item>;
    type State = State;
    type Output = AgentActivityResponse<HeaderHash>;

    fn query(&self) -> String {
        "
            SELECT Header.hash, DhtOp.validation_status, Header.blob AS header_blob,
            DhtOp.when_integrated
            FROM Header
            JOIN DhtOp ON DhtOp.header_hash = Header.hash
            WHERE Header.author = :author
            AND DhtOp.type = :op_type
            ORDER BY Header.seq ASC
        "
        .to_string()
    }

    fn params(&self) -> Vec<holochain_state::query::Params> {
        (named_params! {
            ":author": self.agent,
            ":op_type": DhtOpType::RegisterAgentActivity,
        })
        .to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(Default::default())
    }

    fn as_filter(&self) -> Box<dyn Fn(&QueryData<Self>) -> bool> {
        unimplemented!("This query should not be used with the scratch")
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Item>> {
        Arc::new(move |row| {
            let validation_status: Option<ValidationStatus> = row.get("validation_status")?;
            let hash: HeaderHash = row.get("hash")?;
            from_blob::<SignedHeader>(row.get("header_blob")?).and_then(|header| {
                let integrated: Option<Timestamp> = row.get("when_integrated")?;
                let header = HeaderHashed::with_pre_hashed(header.0, hash);
                let item = if integrated.is_some() {
                    Item::Integrated(header)
                } else {
                    Item::Pending(header)
                };
                Ok(Judged::raw(item, validation_status))
            })
        })
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        let status = item.validation_status();
        match (status, item.data) {
            (Some(ValidationStatus::Valid), Item::Integrated(header)) => {
                let seq = header.header_seq();
                if state.status.is_none() {
                    let fork = state.valid.last().and_then(|v| {
                        if seq == v.header_seq() {
                            Some(v)
                        } else {
                            None
                        }
                    });
                    if let Some(fork) = fork {
                        state.status = Some(ChainStatus::Forked(ChainFork {
                            fork_seq: seq,
                            first_header: header.as_hash().clone(),
                            second_header: fork.as_hash().clone(),
                        }));
                    }
                }

                state.valid.push(header);
            }
            (Some(ValidationStatus::Rejected), Item::Integrated(header)) => {
                if state.status.is_none() {
                    state.status = Some(ChainStatus::Invalid(ChainHead {
                        header_seq: header.header_seq(),
                        hash: header.as_hash().clone(),
                    }));
                }
                state.rejected.push(header);
            }
            (_, Item::Pending(data)) => state.pending.push(data),
            _ => (),
        }
        Ok(state)
    }

    fn render<S>(&self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Store,
    {
        let highest_observed = compute_highest_observed(&state);
        let status = compute_chain_status(&state);

        let valid = state.valid;
        let rejected = state.rejected;
        let valid_activity = if self.options.include_valid_activity {
            let valid = valid
                .into_iter()
                .filter(|h| self.filter.check(h.as_content()))
                .map(|h| (h.header_seq(), h.into_hash()))
                .collect();
            ChainItems::Hashes(valid)
        } else {
            ChainItems::NotRequested
        };
        let rejected_activity = if self.options.include_rejected_activity {
            let rejected = rejected
                .into_iter()
                .filter(|h| self.filter.check(h.as_content()))
                .map(|h| (h.header_seq(), h.into_hash()))
                .collect();
            ChainItems::Hashes(rejected)
        } else {
            ChainItems::NotRequested
        };

        Ok(AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity,
            rejected_activity,
            status,
            highest_observed,
        })
    }
}

fn compute_chain_status(state: &State) -> ChainStatus {
    state.status.clone().unwrap_or_else(|| {
        if state.valid.is_empty() && state.rejected.is_empty() {
            ChainStatus::Empty
        } else {
            let last = state.valid.last().expect("Safe due to is_empty check");
            ChainStatus::Valid(ChainHead {
                header_seq: last.header_seq(),
                hash: last.as_hash().clone(),
            })
        }
    })
}

fn compute_highest_observed(state: &State) -> Option<HighestObserved> {
    let mut highest_observed = None;
    let mut hashes = Vec::new();
    let mut check_highest = |seq: u32, hash: &HeaderHash| {
        if highest_observed.is_none() {
            highest_observed = Some(seq);
            hashes.push(hash.clone());
        } else {
            let last = highest_observed
                .as_mut()
                .expect("Safe due to none check above");
            match seq.cmp(last) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => hashes.push(hash.clone()),
                std::cmp::Ordering::Greater => {
                    hashes.clear();
                    hashes.push(hash.clone());
                    *last = seq;
                }
            }
        }
    };
    if let Some(valid) = state.valid.last() {
        check_highest(valid.header_seq(), valid.as_hash());
    }
    if let Some(rejected) = state.rejected.last() {
        check_highest(rejected.header_seq(), rejected.as_hash());
    }
    if let Some(pending) = state.pending.last() {
        check_highest(pending.header_seq(), pending.as_hash());
    }
    highest_observed.map(|header_seq| HighestObserved {
        header_seq,
        hash: hashes,
    })
}
