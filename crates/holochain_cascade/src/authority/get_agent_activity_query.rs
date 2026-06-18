use crate::agent_activity::compute_chain_status;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_p2p::event::GetActivityOptions;
use holochain_state::prelude::ActionSequenceAndHash;
use holochain_state::query::StateQueryResult;
use holochain_types::prelude::{
    ActionHashedContainer, AgentActivityResponse, ChainItems, ChainItemsSource,
};
use holochain_zome_types::prelude::{
    Action, ChainFork, ChainHead, ChainQueryFilter, ChainStatus, HasValidationStatus,
    HighestObserved, Judged, SignedWarrant, ValidationStatus,
};

pub mod hashes;
pub mod must_get_agent_activity;
pub mod records;

#[derive(Debug)]
pub struct State<T> {
    pub(super) valid: Vec<T>,
    pub(super) rejected: Vec<T>,
    pub(super) pending: Vec<T>,
    pub(super) warrants: Vec<SignedWarrant>,
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

#[allow(clippy::large_enum_variant)]
pub enum Item<T> {
    Integrated(T),
    Pending(T),
    Warrant(SignedWarrant),
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
/// The function searches the valid, rejected, and pending action lists for the highest sequence
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

    // A chain whose head is a `CloseChain` action is reported as `Closed`. This
    // only upgrades a `Valid` status; `compute_chain_status` returns `Forked`
    // and `Invalid` ahead of `Valid`, so higher-priority states are preserved.
    let status = match status {
        ChainStatus::Valid(head)
            if matches!(valid.last().map(|v| v.action()), Some(Action::CloseChain(_))) =>
        {
            ChainStatus::Closed(head)
        }
        other => other,
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::*;
    use holochain_p2p::event::GetActivityOptions;
    use holochain_types::prelude::Record;
    use holochain_zome_types::prelude::*;

    fn signed_record(action: Action) -> Record {
        let shh = SignedActionHashed::with_presigned(
            ActionHashed::from_content_sync(action),
            fixt!(Signature),
        );
        Record::new(shh, None)
    }

    fn dna(agent: &AgentPubKey) -> Record {
        let mut dna = fixt!(Dna);
        dna.author = agent.clone();
        signed_record(Action::Dna(dna))
    }

    fn create(agent: &AgentPubKey, seq: u32) -> Record {
        let mut create = fixt!(Create);
        create.author = agent.clone();
        create.action_seq = seq;
        signed_record(Action::Create(create))
    }

    fn close(agent: &AgentPubKey, seq: u32) -> Record {
        let mut close = fixt!(CloseChain);
        close.author = agent.clone();
        close.action_seq = seq;
        signed_record(Action::CloseChain(close))
    }

    fn state_of(valid: Vec<Record>, rejected: Vec<Record>) -> State<Record> {
        State {
            valid,
            rejected,
            pending: vec![],
            warrants: vec![],
            status: None,
        }
    }

    fn render_status(state: State<Record>, agent: AgentPubKey) -> ChainStatus {
        let options = GetActivityOptions {
            include_valid_activity: true,
            ..Default::default()
        };
        render(state, agent, &ChainQueryFilter::new(), &options)
            .unwrap()
            .status
    }

    #[test]
    fn closed_chain_reports_closed() {
        let agent = fixt!(AgentPubKey);
        let close_record = close(&agent, 2);
        let close_head = ChainHead {
            action_seq: 2,
            hash: close_record.action_address().clone(),
        };
        let state = state_of(
            vec![dna(&agent), create(&agent, 1), close_record],
            vec![],
        );
        assert_eq!(render_status(state, agent), ChainStatus::Closed(close_head));
    }

    #[test]
    fn forked_and_closed_reports_forked() {
        let agent = fixt!(AgentPubKey);
        let state = state_of(
            vec![
                dna(&agent),
                create(&agent, 1),
                create(&agent, 1), // fork at seq 1
                close(&agent, 2),
            ],
            vec![],
        );
        assert!(matches!(
            render_status(state, agent),
            ChainStatus::Forked(_)
        ));
    }

    #[test]
    fn invalid_and_closed_reports_invalid() {
        let agent = fixt!(AgentPubKey);
        let state = state_of(
            vec![dna(&agent), create(&agent, 1), close(&agent, 2)],
            vec![create(&agent, 1)], // a rejected action
        );
        assert!(matches!(
            render_status(state, agent),
            ChainStatus::Invalid(_)
        ));
    }
}
