use std::collections::HashSet;

use super::*;
use holochain_p2p::actor::GetActivityOptions;

pub(crate) fn merge_activities(
    agent: AgentPubKey,
    options: &GetActivityOptions,
    results: Vec<AgentActivityResponse<HeaderHash>>,
) -> CascadeResult<AgentActivityResponse<HeaderHash>> {
    if !options.include_rejected_activity && !options.include_valid_activity {
        return Ok(merge_status_only(agent, results));
    }
    Ok(merge_hashes(agent, options, results))
}

fn merge_hashes(
    agent: AgentPubKey,
    options: &GetActivityOptions,
    results: Vec<AgentActivityResponse<HeaderHash>>,
) -> AgentActivityResponse<HeaderHash> {
    let mut valid = HashSet::new();
    let mut rejected = HashSet::new();
    let mut merged_highest_observed = None;
    for result in results {
        let AgentActivityResponse {
            agent: the_agent,
            highest_observed,
            valid_activity,
            rejected_activity,
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
                let c = if a.header_seq > b.header_seq { a } else { b };
                merged_highest_observed = Some(c);
            }
        }

        match valid_activity {
            ChainItems::Full(_) => {
                // TODO: BACKLOG: Currently not handling full headers from
                // the activity authority.
            }
            ChainItems::Hashes(hashes) => {
                valid.extend(hashes);
            }
            ChainItems::NotRequested => {}
        }
        match rejected_activity {
            ChainItems::Full(_) => {
                // TODO: BACKLOG: Currently not handling full headers from
                // the activity authority.
            }
            ChainItems::Hashes(hashes) => {
                rejected.extend(hashes);
            }
            ChainItems::NotRequested => {}
        }
    }

    let (status, valid, rejected) = compute_chain_status(valid, rejected);
    let valid_activity = if options.include_valid_activity {
        ChainItems::Hashes(valid)
    } else {
        ChainItems::NotRequested
    };
    let rejected_activity = if options.include_rejected_activity {
        ChainItems::Hashes(rejected)
    } else {
        ChainItems::NotRequested
    };
    AgentActivityResponse {
        status,
        agent,
        valid_activity,
        rejected_activity,
        highest_observed: merged_highest_observed,
    }
}

type ValidHashes = Vec<(u32, HeaderHash)>;
type RejectedHashes = Vec<(u32, HeaderHash)>;

fn compute_chain_status(
    valid: HashSet<(u32, HeaderHash)>,
    rejected: HashSet<(u32, HeaderHash)>,
) -> (ChainStatus, ValidHashes, RejectedHashes) {
    let mut valid: Vec<_> = valid.into_iter().collect();
    let mut rejected: Vec<_> = rejected.into_iter().collect();
    // Sort ascending.
    valid.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    rejected.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut valid_out = Vec::with_capacity(valid.len());
    let mut status = None;
    for (seq, hash) in valid {
        if status.is_none() {
            let fork = valid_out
                .last()
                .and_then(|v: &(u32, HeaderHash)| if seq == v.0 { Some(v) } else { None });
            if let Some(fork) = fork {
                status = Some(ChainStatus::Forked(ChainFork {
                    fork_seq: seq,
                    first_header: hash.clone(),
                    second_header: fork.1.clone(),
                }));
            }
        }

        valid_out.push((seq, hash));
    }

    if status.is_none() {
        if let Some((s, h)) = rejected.first() {
            status = Some(ChainStatus::Invalid(ChainHead {
                header_seq: *s,
                hash: h.clone(),
            }));
        }
    }

    let status = status.unwrap_or_else(|| {
        if valid_out.is_empty() && rejected.is_empty() {
            ChainStatus::Empty
        } else {
            let last = valid_out.last().expect("Safe due to is_empty check");
            ChainStatus::Valid(ChainHead {
                header_seq: last.0,
                hash: last.1.clone(),
            })
        }
    });
    (status, valid_out, rejected)
}

fn merge_status_only(
    agent: AgentPubKey,
    results: Vec<AgentActivityResponse<HeaderHash>>,
) -> AgentActivityResponse<HeaderHash> {
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
                let c = if a.header_seq > b.header_seq { a } else { b };
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
                    let c = if a.header_seq > b.header_seq { a } else { b };
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
                    let c = if a.header_seq < b.header_seq { a } else { b };
                    merged_status = Some(ChainStatus::Invalid(c));
                }
                (ChainStatus::Forked(a), ChainStatus::Invalid(b)) => {
                    if a.fork_seq < b.header_seq {
                        merged_status = Some(ChainStatus::Forked(a));
                    } else {
                        merged_status = Some(ChainStatus::Invalid(b));
                    };
                }
                (ChainStatus::Invalid(a), ChainStatus::Forked(b)) => {
                    if a.header_seq < b.fork_seq {
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
        highest_observed: merged_highest_observed,
    }
}
