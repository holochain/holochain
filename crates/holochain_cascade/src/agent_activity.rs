use super::*;
use holochain_p2p::actor::GetActivityOptions;

#[allow(clippy::result_large_err)]
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
    let mut merged_result_status = ChainStatus::Empty;
    let mut valid = if options.include_valid_activity {
        if options.include_full_records {
            ChainItems::Full(Vec::new())
        } else {
            ChainItems::Hashes(Vec::new())
        }
    } else {
        ChainItems::NotRequested
    };
    let mut rejected = if options.include_rejected_activity {
        if options.include_full_records {
            ChainItems::Full(Vec::new())
        } else {
            ChainItems::Hashes(Vec::new())
        }
    } else {
        ChainItems::NotRequested
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
            status: result_status,
        } = result;

        if the_agent != agent {
            continue;
        }

        merged_result_status = combine_chain_status(merged_result_status, result_status);

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
                (
                    ChainItems::Full(mut v),
                    ChainItems::Full(mut r),
                    ChainItems::Full(valid),
                    ChainItems::Full(rejected),
                ) if options.include_full_records => {
                    v.extend(valid);
                    r.extend(rejected);
                    let (status, valid, rejected) =
                        compute_chain_status(v.into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                (
                    ChainItems::Hashes(mut v),
                    ChainItems::Hashes(mut r),
                    ChainItems::Hashes(valid),
                    ChainItems::Hashes(rejected),
                ) => {
                    v.extend(valid);
                    r.extend(rejected);
                    let (status, valid, rejected) =
                        compute_chain_status(v.into_iter(), r.into_iter());

                    (status, valid.to_chain_items(), rejected.to_chain_items())
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (
                        ChainStatus::Empty,
                        ChainItems::NotRequested,
                        ChainItems::NotRequested,
                    )
                }
            }
        } else if options.include_valid_activity {
            match (valid, rejected, valid_activity, rejected_activity) {
                (ChainItems::Full(mut v), _, ChainItems::Full(valid), _)
                    if options.include_full_records =>
                {
                    v.extend(valid);
                    let (status, valid, rejected) =
                        compute_chain_status(v.into_iter(), Vec::with_capacity(0).into_iter());

                    (
                        status,
                        valid.to_chain_items(),
                        if rejected.is_empty() {
                            ChainItems::NotRequested
                        } else {
                            rejected.to_chain_items()
                        },
                    )
                }
                (ChainItems::Hashes(mut v), _, ChainItems::Hashes(valid), _) => {
                    v.extend(valid);
                    let (status, valid, rejected) =
                        compute_chain_status(v.into_iter(), Vec::with_capacity(0).into_iter());

                    (
                        status,
                        valid.to_chain_items(),
                        if rejected.is_empty() {
                            ChainItems::NotRequested
                        } else {
                            rejected.to_chain_items()
                        },
                    )
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (
                        ChainStatus::Empty,
                        ChainItems::NotRequested,
                        ChainItems::NotRequested,
                    )
                }
            }
        } else if options.include_rejected_activity {
            match (valid, rejected, valid_activity, rejected_activity) {
                (_, ChainItems::Full(mut r), _, ChainItems::Full(rejected))
                    if options.include_full_records =>
                {
                    r.extend(rejected);
                    let (status, valid, rejected) =
                        compute_chain_status(Vec::with_capacity(0).into_iter(), r.into_iter());

                    (
                        status,
                        if valid.is_empty() {
                            ChainItems::NotRequested
                        } else {
                            valid.to_chain_items()
                        },
                        rejected.to_chain_items(),
                    )
                }
                (_, ChainItems::Hashes(mut r), _, ChainItems::Hashes(rejected)) => {
                    r.extend(rejected);
                    let (status, valid, rejected) =
                        compute_chain_status(Vec::with_capacity(0).into_iter(), r.into_iter());

                    (
                        status,
                        if valid.is_empty() {
                            ChainItems::NotRequested
                        } else {
                            valid.to_chain_items()
                        },
                        rejected.to_chain_items(),
                    )
                }
                e => {
                    warn!("Invalid combination of chain items in merge_hashes: {e:?}");
                    (
                        ChainStatus::Empty,
                        ChainItems::NotRequested,
                        ChainItems::NotRequested,
                    )
                }
            }
        } else {
            (
                ChainStatus::Empty,
                ChainItems::NotRequested,
                ChainItems::NotRequested,
            )
        };

        valid = v;
        rejected = r;
        status = s;
    }

    let status = combine_chain_status(status, merged_result_status);

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
    valid: impl Iterator<Item = T>,
    rejected: impl Iterator<Item = T>,
) -> (ChainStatus, Vec<T>, Vec<T>) {
    let mut valid: Vec<_> = valid.collect();
    let mut rejected: Vec<_> = rejected.collect();

    // Sort ascending.
    valid.sort_unstable_by_key(|a| a.action_seq());
    rejected.sort_unstable_by_key(|a| a.action_seq());

    let mut valid_out = Vec::with_capacity(valid.len());
    let mut status = None;

    for current in valid {
        if status.is_none() {
            let fork = valid_out.last().and_then(|v: &T| {
                if current.action_seq() == v.action_seq() {
                    Some(v)
                } else {
                    None
                }
            });

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
    let status = status.unwrap_or_else(|| match (valid_out.last(), rejected.first()) {
        (None, None) => ChainStatus::Empty,
        (Some(v), None) => ChainStatus::Valid(ChainHead {
            action_seq: v.action_seq(),
            hash: v.address().clone(),
        }),
        (None, Some(r)) => ChainStatus::Invalid(ChainHead {
            action_seq: r.action_seq(),
            hash: r.address().clone(),
        }),
        (Some(_), Some(r)) => ChainStatus::Invalid(ChainHead {
            action_seq: r.action_seq(),
            hash: r.address().clone(),
        }),
    });

    (status, valid_out, rejected)
}

/// Combine two chain statuses, keeping the higher-priority one.
///
/// Priority, lowest to highest: `Empty` < `Valid` < `Closed` < `Forked`/`Invalid`.
/// Same-rank tie-breaks: `Valid`/`Valid` and `Closed`/`Closed` keep the higher
/// `action_seq`; `Forked`/`Forked` keeps the lower `fork_seq`; `Invalid`/`Invalid`
/// keeps the lower `action_seq`; `Forked` vs `Invalid` keeps whichever has the
/// lower sequence number, and `Forked` on an exact tie so the result is the same
/// regardless of operand order.
fn combine_chain_status(a: ChainStatus, b: ChainStatus) -> ChainStatus {
    use ChainStatus::*;
    match (a, b) {
        (Empty, other) | (other, Empty) => other,

        (Valid(a), Valid(b)) => {
            if a.action_seq > b.action_seq {
                Valid(a)
            } else {
                Valid(b)
            }
        }

        (Closed(c), Valid(_)) | (Valid(_), Closed(c)) => Closed(c),
        (Closed(a), Closed(b)) => {
            if a.action_seq > b.action_seq {
                Closed(a)
            } else {
                Closed(b)
            }
        }

        (Forked(c), Valid(_))
        | (Valid(_), Forked(c))
        | (Forked(c), Closed(_))
        | (Closed(_), Forked(c)) => Forked(c),
        (Invalid(c), Valid(_))
        | (Valid(_), Invalid(c))
        | (Invalid(c), Closed(_))
        | (Closed(_), Invalid(c)) => Invalid(c),

        (Forked(a), Forked(b)) => {
            if a.fork_seq < b.fork_seq {
                Forked(a)
            } else {
                Forked(b)
            }
        }
        (Invalid(a), Invalid(b)) => {
            if a.action_seq < b.action_seq {
                Invalid(a)
            } else {
                Invalid(b)
            }
        }
        (Forked(a), Invalid(b)) => {
            // `<=` so an exact tie resolves to `Forked` in both operand orders
            // (the `(Invalid, Forked)` arm below also yields `Forked` on a tie).
            if a.fork_seq <= b.action_seq {
                Forked(a)
            } else {
                Invalid(b)
            }
        }
        (Invalid(a), Forked(b)) => {
            if a.action_seq < b.fork_seq {
                Invalid(a)
            } else {
                Forked(b)
            }
        }
    }
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
        merged_status = Some(match merged_status.take() {
            Some(last) => combine_chain_status(status, last),
            None => status,
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::ActionHash;
    use holochain_zome_types::prelude::{ChainFork, ChainHead, ChainStatus};

    fn head(seq: u32) -> ChainHead {
        ChainHead {
            action_seq: seq,
            hash: ActionHash::from_raw_32(vec![seq as u8; 32]),
        }
    }

    fn fork(seq: u32) -> ChainFork {
        ChainFork {
            fork_seq: seq,
            first_action: ActionHash::from_raw_32(vec![seq as u8; 32]),
            second_action: ActionHash::from_raw_32(vec![(seq + 1) as u8; 32]),
        }
    }

    #[test]
    fn merge_full_carries_closed_through_hashes() {
        use holo_hash::fixt::*;
        use ChainStatus::*;

        let agent = ::fixt::prelude::fixt!(AgentPubKey);
        let options = GetActivityOptions {
            include_valid_activity: true,
            ..Default::default()
        };

        let response = AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::Hashes(vec![
                (0, ::fixt::prelude::fixt!(ActionHash)),
                (1, ::fixt::prelude::fixt!(ActionHash)),
                (2, head(2).hash),
            ]),
            rejected_activity: ChainItems::NotRequested,
            status: Closed(head(2)),
            highest_observed: None,
            warrants: vec![],
        };

        let merged = merge_activity_responses(agent, &options, vec![response]);
        assert_eq!(merged.status, Closed(head(2)));
    }

    #[test]
    fn merge_full_forked_beats_closed() {
        use holo_hash::fixt::*;
        use ChainStatus::*;

        let agent = ::fixt::prelude::fixt!(AgentPubKey);
        let options = GetActivityOptions {
            include_valid_activity: true,
            ..Default::default()
        };

        let response = AgentActivityResponse {
            agent: agent.clone(),
            valid_activity: ChainItems::Hashes(vec![
                (0, ::fixt::prelude::fixt!(ActionHash)),
                (2, head(2).hash),
            ]),
            rejected_activity: ChainItems::NotRequested,
            status: Forked(fork(1)),
            highest_observed: None,
            warrants: vec![],
        };

        let merged = merge_activity_responses(agent, &options, vec![response]);
        assert!(matches!(merged.status, Forked(_)));
    }

    #[test]
    fn combine_status_priority() {
        use ChainStatus::*;

        // Empty is the identity element.
        assert_eq!(combine_chain_status(Empty, Empty), Empty);
        assert_eq!(combine_chain_status(Empty, Valid(head(3))), Valid(head(3)));
        assert_eq!(
            combine_chain_status(Closed(head(3)), Empty),
            Closed(head(3))
        );

        // Closed outranks Valid; Forked/Invalid outrank Closed.
        assert_eq!(
            combine_chain_status(Valid(head(5)), Closed(head(2))),
            Closed(head(2))
        );
        assert_eq!(
            combine_chain_status(Closed(head(9)), Forked(fork(4))),
            Forked(fork(4))
        );
        assert_eq!(
            combine_chain_status(Invalid(head(4)), Closed(head(9))),
            Invalid(head(4))
        );

        // Same-rank tie-breaks (must match prior behavior).
        assert_eq!(
            combine_chain_status(Valid(head(2)), Valid(head(7))),
            Valid(head(7))
        );
        assert_eq!(
            combine_chain_status(Closed(head(7)), Closed(head(2))),
            Closed(head(7))
        );
        assert_eq!(
            combine_chain_status(Forked(fork(6)), Forked(fork(2))),
            Forked(fork(2))
        );
        assert_eq!(
            combine_chain_status(Invalid(head(6)), Invalid(head(2))),
            Invalid(head(2))
        );
        assert_eq!(
            combine_chain_status(Forked(fork(2)), Invalid(head(6))),
            Forked(fork(2))
        );
        assert_eq!(
            combine_chain_status(Invalid(head(6)), Forked(fork(2))),
            Forked(fork(2))
        );

        // Forked vs Invalid at an equal sequence resolves to Forked regardless
        // of operand order (deterministic across merge folds).
        assert_eq!(
            combine_chain_status(Forked(fork(4)), Invalid(head(4))),
            Forked(fork(4))
        );
        assert_eq!(
            combine_chain_status(Invalid(head(4)), Forked(fork(4))),
            Forked(fork(4))
        );
    }
}
