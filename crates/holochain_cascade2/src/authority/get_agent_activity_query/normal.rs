use holo_hash::*;
use holochain_p2p::event::GetActivityOptions;
use holochain_sqlite::rusqlite::*;
use holochain_state::{prelude::*, query::QueryData};
use holochain_zome_types::Judged;
use holochain_zome_types::*;
use std::fmt::Debug;

use crate::authority::*;

#[derive(Debug, Clone)]
pub struct GetAgentActivityQuery {
    agent: AgentPubKey,
    filter: Filter,
    options: GetActivityOptions,
}

impl GetAgentActivityQuery {
    pub fn new(agent: AgentPubKey, filter: ChainQueryFilter, options: GetActivityOptions) -> Self {
        let filter = Filter {
            start: filter.sequence_range.clone().map(|r| r.start),
            end: filter.sequence_range.clone().map(|r| r.end),
            header_type: filter.header_type,
            entry_type: filter.entry_type,
        };
        Self {
            agent,
            filter,
            options,
        }
    }
}

#[derive(Debug, Clone)]
struct Filter {
    start: Option<u32>,
    end: Option<u32>,
    header_type: Option<HeaderType>,
    entry_type: Option<EntryType>,
}

#[derive(Debug, Default)]
pub struct State {
    valid: Vec<(u32, HeaderHash)>,
    rejected: Vec<(u32, HeaderHash)>,
    pending: Vec<(u32, HeaderHash)>,
    status: Option<ChainStatus>,
}

#[derive(Debug)]
pub enum Item {
    Integrated((u32, HeaderHash)),
    Pending((u32, HeaderHash)),
}

impl Query for GetAgentActivityQuery {
    type Item = Judged<Item>;
    type State = State;
    type Output = AgentActivityResponse<HeaderHash>;

    fn query(&self) -> String {
        "
            SELECT Header.hash, DhtOp.validation_status, Header.seq,
            DhtOp.when_integrated
            FROM Header
            JOIN DhtOp ON DhtOp.header_hash = Header.hash
            WHERE Header.author = :author
            AND DhtOp.type = :op_type
            AND
            (:range_start IS NULL OR Header.seq >= :range_start)
            AND
            (:range_end IS NULL OR Header.seq < :range_end)
            ORDER BY Header.seq ASC 
        "
        .to_string()
        // AND (:hash_low IS NULL OR H.seq >= (SELECT seq FROM Header WHERE hash = :hash_low))
        // AND H.seq <= (SELECT seq FROM Header WHERE hash = :hash_high)
    }

    fn params(&self) -> Vec<Params> {
        (named_params! {
            ":author": self.agent,
            ":range_start": self.filter.start,
            ":range_end": self.filter.end,
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
            let validation_status: ValidationStatus = row.get("validation_status")?;
            let hash: HeaderHash = row.get("hash")?;
            let seq: u32 = row.get("seq")?;
            let integrated: Option<i32> = row.get("when_integrated")?;
            let item = if integrated.is_some() {
                Item::Integrated((seq, hash))
            } else {
                Item::Pending((seq, hash))
            };
            Ok(Judged::new(item, validation_status))
        })
    }

    fn fold(&self, mut state: Self::State, item: Self::Item) -> StateQueryResult<Self::State> {
        let status = item.validation_status();
        match (status, item.data) {
            (Some(ValidationStatus::Valid), Item::Integrated(data)) => {
                if state.status.is_none() {
                    let fork =
                        state
                            .valid
                            .last()
                            .and_then(|v| if data.0 == v.0 { Some(v) } else { None });
                    if let Some(fork) = fork {
                        state.status = Some(ChainStatus::Forked(ChainFork {
                            fork_seq: data.0,
                            first_header: data.1.clone(),
                            second_header: fork.1.clone(),
                        }));
                    }
                }

                state.valid.push(data);
            }
            (Some(ValidationStatus::Rejected), Item::Integrated(data)) => {
                if state.status.is_none() {
                    state.status = Some(ChainStatus::Invalid(ChainHead {
                        header_seq: data.0,
                        hash: data.1.clone(),
                    }));
                }
                state.rejected.push(data);
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
            ChainItems::Hashes(valid)
        } else {
            ChainItems::NotRequested
        };
        let rejected_activity = if self.options.include_rejected_activity {
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
                header_seq: last.0,
                hash: last.1.clone(),
            })
        }
    })
}

fn compute_highest_observed(state: &State) -> Option<HighestObserved> {
    let mut highest_observed = None;
    let mut hashes = Vec::new();
    let mut check_highest = |i: &(u32, HeaderHash)| {
        if highest_observed.is_none() {
            highest_observed = Some(i.0);
            hashes.push(i.1.clone());
        } else {
            let last = highest_observed
                .as_mut()
                .expect("Safe due to none check above");
            match i.0.cmp(last) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => hashes.push(i.1.clone()),
                std::cmp::Ordering::Greater => {
                    hashes.clear();
                    hashes.push(i.1.clone());
                    *last = i.0;
                }
            }
        }
    };
    if let Some(valid) = state.valid.last() {
        check_highest(valid);
    }
    if let Some(rejected) = state.rejected.last() {
        check_highest(rejected);
    }
    if let Some(pending) = state.pending.last() {
        check_highest(pending);
    }
    highest_observed.map(|header_seq| HighestObserved {
        header_seq,
        hash: hashes,
    })
}
