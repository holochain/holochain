use super::*;
use holochain_p2p::actor::GetActivityOptions;

pub(crate) fn merge_activities(
    agent: AgentPubKey,
    options: &GetActivityOptions,
    results: Vec<AgentActivityResponse>,
) -> CascadeResult<AgentActivityResponse> {
    if !options.include_rejected_activity
        && !options.include_valid_activity
        && !options.include_warrants
    {
        return Ok(merge_status_only(agent, results));
    }
    Ok(merge_activity_responses(agent, options, results))
}

fn merge_activity_responses(
    agent: AgentPubKey,
    options: &GetActivityOptions,
    results: Vec<AgentActivityResponse>,
) -> AgentActivityResponse {
    let mut status = ChainStatus::Empty;
    let mut valid = if options.include_full_records {
        ChainItems::FullRecords(Vec::new())
    } else if options.include_full_actions {
        ChainItems::FullActions(Vec::new())
    } else {
        ChainItems::Hashes(Vec::new())
    };
    let mut rejected = if options.include_full_records {
        ChainItems::FullRecords(Vec::new())
    } else if options.include_full_actions {
        ChainItems::FullActions(Vec::new())
    } else {
        ChainItems::Hashes(Vec::new())
    };
    let mut warrants = Vec::new();
    let mut merged_highest_observed = None;
    for result in results {
        let AgentActivityResponse {
            agent: the_agent,
            highest_observed,
            valid_activity,
            rejected_activity,
            warrants: these_warrants,
            status: _,
        } = result;

        if the_agent != agent {
            continue;
        }

        warrants.extend(these_warrants);

        match (merged_highest_observed.take(), highest_observed) {
            (None, None) => {}
            (Some(h), None) | (None, Some(h)) => {
                merged_highest_observed = Some(h);
            }
            (Some(a), Some(b)) => {
                let c = if a.action_seq > b.action_seq { a } else { b };
                merged_highest_observed = Some(c);
            }
        }

        let (s, v, r) = if options.include_valid_activity && options.include_rejected_activity {
            match (valid, rejected, valid_activity, rejected_activity) {
                (ChainItems::FullRecords(mut v), ChainItems::FullRecords(mut r), ChainItems::FullRecords(valid), ChainItems::FullRecords(rejected)) if options.include_full_records => {
                    v.extend(valid);
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (ChainItems::FullActions(mut v), ChainItems::FullActions(mut r), ChainItems::FullActions(valid), ChainItems::FullActions(rejected)) if options.include_full_actions => {
                    v.extend(valid);
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (ChainItems::Hashes(mut v), ChainItems::Hashes(mut r), ChainItems::Hashes(valid), ChainItems::Hashes(rejected)) => {
                    v.extend(valid);
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), r.into_iter());

                    (status, ChainItems::Hashes(valid), ChainItems::Hashes(rejected))
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (ChainStatus::Empty, ChainItems::NotRequested, ChainItems::NotRequested)
                }
            }
        } else if options.include_valid_activity {
            match (valid, rejected, valid_activity, rejected_activity) {
                (ChainItems::FullRecords(mut v), _, ChainItems::FullRecords(valid), _) if options.include_full_records => {
                    v.extend(valid);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), Vec::with_capacity(0).into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (ChainItems::FullActions(mut v), _, ChainItems::FullActions(valid), _) if options.include_full_actions => {
                    v.extend(valid);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), Vec::with_capacity(0).into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (ChainItems::Hashes(mut v), _, ChainItems::Hashes(valid), _) => {
                    v.extend(valid);
                    let (status, valid, rejected) = compute_chain_status(v.into_iter(), Vec::with_capacity(0).into_iter());

                    (status, ChainItems::Hashes(valid), ChainItems::Hashes(rejected))
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (ChainStatus::Empty, ChainItems::NotRequested, ChainItems::NotRequested)
                }
            }
        } else if options.include_rejected_activity {
            match (valid, rejected, valid_activity, rejected_activity) {
                (_, ChainItems::FullRecords(mut r), _, ChainItems::FullRecords(rejected)) if options.include_full_records => {
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(Vec::with_capacity(0).into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (_, ChainItems::FullActions(mut r), _, ChainItems::FullActions(rejected)) if options.include_full_actions => {
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(Vec::with_capacity(0).into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (_, ChainItems::Hashes(mut r), _, ChainItems::Hashes(rejected)) => {
                    r.extend(rejected);
                    let (status, valid, rejected) = compute_chain_status(Vec::with_capacity(0).into_iter(), r.into_iter());

                    (status, ChainItems::Hashes(valid), ChainItems::Hashes(rejected))
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (ChainStatus::Empty, ChainItems::NotRequested, ChainItems::NotRequested)
                }
            }
        } else {
            (ChainStatus::Empty, ChainItems::NotRequested, ChainItems::NotRequested)
        };

        valid = v;
        rejected = r;
        status = s;
    }

    AgentActivityResponse {
        status,
        agent,
        valid_activity: valid,
        rejected_activity: rejected,
        warrants,
        highest_observed: merged_highest_observed,
    }
}

pub(crate) fn compute_chain_status<T: ActionSequenceAndHash>(
    valid: impl Iterator<Item=T>,
    rejected: impl Iterator<Item=T>,
) -> (ChainStatus, Vec<T>, Vec<T>) {
    let mut valid: Vec<_> = valid.collect();
    let mut rejected: Vec<_> = rejected.collect();

    // Sort ascending.
    valid.sort_unstable_by(|a, b| a.action_seq().cmp(&b.action_seq()));
    rejected.sort_unstable_by(|a, b| a.action_seq().cmp(&b.action_seq()));

    let mut valid_out = Vec::with_capacity(valid.len());
    let mut status = None;

    for current in valid {
        if status.is_none() {
            let fork = valid_out
                .last()
                .and_then(|v: &T| if current.action_seq() == v.action_seq() { Some(v) } else { None });

            if let Some(fork) = fork {
                status = Some(ChainStatus::Forked(ChainFork {
                    fork_seq: current.action_seq(),
                    first_action: current.address().clone(),
                    second_action: fork.address().clone(),
                }));
            }
        }

        valid_out.push(current);
    }

    // The chain status will have been set if we found a fork, otherwise decide the status from
    // the last valid and first rejected actions.
    let status = status.unwrap_or_else(|| {
        match (valid_out.last(), rejected.first()) {
            (None, None) => ChainStatus::Empty,
            (Some(v), None) => ChainStatus::Valid(ChainHead {
                action_seq: v.action_seq(),
                hash: v.address().clone(),
            }),
            (None, Some(r)) => ChainStatus::Invalid(ChainHead {
                action_seq: r.action_seq(),
                hash: r.address().clone(),
            }),
            (Some(_), Some(r)) => {
                ChainStatus::Invalid(ChainHead {
                    action_seq: r.action_seq(),
                    hash: r.address().clone(),
                })
            }
        }
    });

    (status, valid_out, rejected)
}

fn merge_status_only(
    agent: AgentPubKey,
    results: Vec<AgentActivityResponse>,
) -> AgentActivityResponse {
    let mut merged_status = None;
    let mut merged_highest_observed = None;
    for result in results {
        let AgentActivityResponse {
            status,
            agent: the_agent,
            highest_observed,
            ..
        } = result;
        if the_agent != agent {
            continue;
        }
        match (merged_highest_observed.take(), highest_observed) {
            (None, None) => {}
            (Some(h), None) | (None, Some(h)) => {
                merged_highest_observed = Some(h);
            }
            (Some(a), Some(b)) => {
                let c = if a.action_seq > b.action_seq { a } else { b };
                merged_highest_observed = Some(c);
            }
        }
        match merged_status.take() {
            Some(last) => match (status, last) {
                (ChainStatus::Empty, ChainStatus::Empty) => {
                    merged_status = Some(ChainStatus::Empty);
                }
                (ChainStatus::Empty, ChainStatus::Valid(c))
                | (ChainStatus::Valid(c), ChainStatus::Empty) => {
                    merged_status = Some(ChainStatus::Valid(c));
                }
                (ChainStatus::Empty, ChainStatus::Forked(c))
                | (ChainStatus::Forked(c), ChainStatus::Empty) => {
                    merged_status = Some(ChainStatus::Forked(c));
                }
                (ChainStatus::Empty, ChainStatus::Invalid(c))
                | (ChainStatus::Invalid(c), ChainStatus::Empty) => {
                    merged_status = Some(ChainStatus::Invalid(c));
                }
                (ChainStatus::Valid(a), ChainStatus::Valid(b)) => {
                    let c = if a.action_seq > b.action_seq { a } else { b };
                    merged_status = Some(ChainStatus::Valid(c));
                }
                (ChainStatus::Valid(_), ChainStatus::Forked(c))
                | (ChainStatus::Forked(c), ChainStatus::Valid(_)) => {
                    // If the valid and forked chain heads are the same then they are in conflict here.
                    // TODO: BACKLOG: When we handle conflicts this should count as a conflict.
                    merged_status = Some(ChainStatus::Forked(c));
                }
                (ChainStatus::Invalid(c), ChainStatus::Valid(_))
                | (ChainStatus::Valid(_), ChainStatus::Invalid(c)) => {
                    // If the valid and invalid chain heads are the same then they are in conflict here.
                    // TODO: BACKLOG: When we handle conflicts this should count as a conflict.
                    merged_status = Some(ChainStatus::Invalid(c));
                }
                (ChainStatus::Forked(a), ChainStatus::Forked(b)) => {
                    let c = if a.fork_seq < b.fork_seq { a } else { b };
                    merged_status = Some(ChainStatus::Forked(c));
                }
                (ChainStatus::Invalid(a), ChainStatus::Invalid(b)) => {
                    let c = if a.action_seq < b.action_seq { a } else { b };
                    merged_status = Some(ChainStatus::Invalid(c));
                }
                (ChainStatus::Forked(a), ChainStatus::Invalid(b)) => {
                    if a.fork_seq < b.action_seq {
                        merged_status = Some(ChainStatus::Forked(a));
                    } else {
                        merged_status = Some(ChainStatus::Invalid(b));
                    };
                }
                (ChainStatus::Invalid(a), ChainStatus::Forked(b)) => {
                    if a.action_seq < b.fork_seq {
                        merged_status = Some(ChainStatus::Invalid(a));
                    } else {
                        merged_status = Some(ChainStatus::Forked(b));
                    };
                }
            },
            None => {
                merged_status = Some(status);
            }
        }
    }
    AgentActivityResponse {
        status: merged_status.unwrap_or(ChainStatus::Empty),
        agent,
        valid_activity: ChainItems::NotRequested,
        rejected_activity: ChainItems::NotRequested,
        warrants: vec![],
        highest_observed: merged_highest_observed,
    }
}
