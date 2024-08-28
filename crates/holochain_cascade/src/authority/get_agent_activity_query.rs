use crate::agent_activity::compute_chain_status;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_p2p::event::GetActivityOptions;
use holochain_state::prelude::ActionSequenceAndHash;
use holochain_state::query::StateQueryResult;
use holochain_types::prelude::{
    ActionHashedContainer, AgentActivityResponse, ChainItems, ChainItemsSource,
};
use holochain_zome_types::prelude::{
    ChainFork, ChainHead, ChainQueryFilter, ChainStatus, HasValidationStatus, HighestObserved,
    Judged, ValidationStatus, Warrant,
};

pub mod actions;
pub mod deterministic;
pub mod hashes;
pub mod must_get_agent_activity;
pub mod records;

#[derive(Debug)]
pub struct State<T> {
    pub(super) valid: Vec<T>,
    pub(super) rejected: Vec<T>,
    pub(super) pending: Vec<T>,
    pub(super) warrants: Vec<Warrant>,
    pub(super) status: Option<ChainStatus>,
}

impl<T> Default for State<T> {
    fn default() -> Self {
        Self {
            valid: Vec::new(),
            rejected: Vec::new(),
            pending: Vec::new(),
            warrants: Vec::new(),
            status: None,
        }
    }
}

#[derive(Debug)]
pub enum Item<T> {
    Integrated(T),
    Pending(T),
    Warrant(Warrant),
}

fn fold<T: ActionHashedContainer>(
    mut state: State<T>,
    item: Judged<Item<T>>,
) -> StateQueryResult<State<T>> {
    let status = item.validation_status();
    match (status, item.data) {
        (Some(ValidationStatus::Valid), Item::Integrated(action)) => {
            let seq = action.action().action_seq();
            if state.status.is_none() {
                let fork = state.valid.last().and_then(|v| {
                    if seq == v.action().action_seq() {
                        Some(v)
                    } else {
                        None
                    }
                });
                if let Some(fork) = fork {
                    state.status = Some(ChainStatus::Forked(ChainFork {
                        fork_seq: seq,
                        first_action: action.action_hash().clone(),
                        second_action: fork.action_hash().clone(),
                    }));
                }
            }

            state.valid.push(action);
        }
        (Some(ValidationStatus::Rejected), Item::Integrated(action)) => {
            if state.status.is_none() {
                state.status = Some(ChainStatus::Invalid(ChainHead {
                    action_seq: action.action().action_seq(),
                    hash: action.action_hash().clone(),
                }));
            }

            state.rejected.push(action);
        }
        (_, Item::Pending(data)) => state.pending.push(data),
        (_, Item::Warrant(warrant)) => state.warrants.push(warrant),
        _ => (),
    }

    Ok(state)
}

/// Find the highest observed sequence number from multiple action lists.
///
/// THe function searches the valid, rejected, and pending action lists for the highest sequence
/// number. If there are multiple actions with the same sequence number, all of their hashes are
/// returned.
fn compute_highest_observed<T: ActionSequenceAndHash>(state: &State<T>) -> Option<HighestObserved> {
    let mut highest_observed = None;
    let mut hashes = Vec::new();
    let mut check_highest = |seq: u32, hash: &ActionHash| {
        if let Some(last) = highest_observed.as_mut() {
            match seq.cmp(last) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => hashes.push(hash.clone()),
                std::cmp::Ordering::Greater => {
                    hashes.clear();
                    hashes.push(hash.clone());
                    *last = seq;
                }
            }
        } else {
            highest_observed = Some(seq);
            hashes.push(hash.clone());
        }
    };
    if let Some(valid) = state.valid.last() {
        check_highest(valid.action_seq(), valid.address());
    }
    if let Some(rejected) = state.rejected.last() {
        check_highest(rejected.action_seq(), rejected.address());
    }
    if let Some(pending) = state.pending.last() {
        check_highest(pending.action_seq(), pending.address());
    }
    highest_observed.map(|action_seq| HighestObserved {
        action_seq,
        hash: hashes,
    })
}

fn render<T>(
    state: State<T>,
    agent: AgentPubKey,
    filter: &ChainQueryFilter,
    options: &GetActivityOptions,
) -> StateQueryResult<AgentActivityResponse>
where
    T: ActionHashedContainer + Clone,
    Vec<T>: ChainItemsSource,
{
    let highest_observed = compute_highest_observed(&state);

    let (status, valid, rejected) = compute_chain_status(
        state.valid.clone().into_iter(),
        state.rejected.clone().into_iter(),
    );

    let valid_activity = if options.include_valid_activity {
        let valid = filter.filter_actions(valid);
        valid.to_chain_items()
    } else {
        ChainItems::NotRequested
    };

    let rejected_activity = if options.include_rejected_activity {
        let rejected = filter.filter_actions(rejected);
        rejected.to_chain_items()
    } else {
        ChainItems::NotRequested
    };

    let warrants = if options.include_warrants {
        state.warrants
    } else {
        vec![]
    };

    Ok(AgentActivityResponse {
        agent,
        valid_activity,
        rejected_activity,
        warrants,
        status,
        highest_observed,
    })
}
