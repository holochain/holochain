use super::*;

/// Rules to make this monotonic
/// - Invalid overwrites valid and any invalids later in the chain.
/// - Later Valid headers overwrite earlier Valid.
/// - If there are two Valid status at the same seq num then insert an Fork.
pub(super) fn add_chain_status(
    prev_status: ChainStatus,
    incoming_status: ChainStatus,
) -> Option<ChainStatus> {
    use ChainStatus::*;
    match (&prev_status, &incoming_status) {
        (Valid(p), Valid(c)) => {
            if p.header_seq == c.header_seq && p.hash != c.hash {
                // Found a fork so insert a fork
                Some(Forked(ChainFork{
                    fork_seq: p.header_seq,
                    first_header: p.hash.clone(),
                    second_header: c.hash.clone(),
                }))
            } else if p == c || p.header_seq > c.header_seq {
                // Both are the same no need to overwrite or
                // Previous is more recent so don't overwrite
                None
            } else {
                // Otherwise overwrite with current
                Some(incoming_status)
            }
        }
        // # Reasons to not overwrite
        // ## Invalid / Forked where the previous is earlier in the chain
        (Invalid(p), Forked(c)) if p.header_seq <= c.fork_seq => None,
        (Invalid(p), Invalid(c)) if p.header_seq <= c.header_seq => None,
        (Forked(p), Invalid(c)) if p.fork_seq <= c.header_seq => None,
        (Forked(p), Forked(c)) if p.fork_seq <= c.fork_seq => None,
        // ## Previous is Invalid / Forked and current is valid
        (Invalid(_), Valid(_)) | (Forked(_), Valid(_))
        // Current is empty
        | (_, Empty) => None,
        // Previous should never be empty
        (Empty, _) => unreachable!("Should never cache an empty status"),
        // The rest are reasons to overwrite
        _ => Some(incoming_status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;

    use ChainStatus::*;

    /// TEST: The following chain status transitions have the proper precedence
    #[test]
    fn add_chain_status_test() {
        // - Invalid overwrites valid
        let prev_status = Valid(ChainHead {
            header_seq: 1,
            hash: fixt!(HeaderHash),
        });
        let incoming_status = Invalid(ChainHead {
            header_seq: 1,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status, incoming_status.clone()),
            Some(incoming_status.clone())
        );

        // - Invalid overwrites any invalids later in the chain.
        let prev_status = Invalid(ChainHead {
            header_seq: 2,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status.clone(), incoming_status.clone()),
            Some(incoming_status.clone())
        );
        // Reverse and expect reverse result
        assert_eq!(
            add_chain_status(incoming_status.clone(), prev_status.clone()),
            None
        );

        // - Invalid overwrites any forks later in the chain.
        let prev_status = Forked(ChainFork {
            fork_seq: 2,
            first_header: fixt!(HeaderHash),
            second_header: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status.clone(), incoming_status.clone()),
            Some(incoming_status.clone())
        );
        // Reverse and expect reverse result
        assert_eq!(
            add_chain_status(incoming_status.clone(), prev_status.clone()),
            None
        );

        // - Forked overwrites any forks later in the chain.
        let prev_status = Forked(ChainFork {
            fork_seq: 2,
            first_header: fixt!(HeaderHash),
            second_header: fixt!(HeaderHash),
        });
        let incoming_status = Forked(ChainFork {
            fork_seq: 1,
            first_header: fixt!(HeaderHash),
            second_header: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status.clone(), incoming_status.clone()),
            Some(incoming_status.clone())
        );
        // Reverse and expect reverse result
        assert_eq!(
            add_chain_status(incoming_status.clone(), prev_status.clone()),
            None
        );

        // - Forked overwrites any invalid later in the chain.
        let prev_status = Invalid(ChainHead {
            header_seq: 2,
            hash: fixt!(HeaderHash),
        });
        let incoming_status = Forked(ChainFork {
            fork_seq: 1,
            first_header: fixt!(HeaderHash),
            second_header: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status.clone(), incoming_status.clone()),
            Some(incoming_status.clone())
        );
        // Reverse and expect reverse result
        assert_eq!(
            add_chain_status(incoming_status.clone(), prev_status.clone()),
            None
        );

        // - Later Valid headers overwrite earlier Valid.
        let prev_status = Valid(ChainHead {
            header_seq: 1,
            hash: fixt!(HeaderHash),
        });
        let incoming_status = Valid(ChainHead {
            header_seq: 2,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(
            add_chain_status(prev_status, incoming_status.clone()),
            Some(incoming_status)
        );

        // - If there are two Valid status at the same seq num then insert an Fork.
        let hashes: Vec<_> = HeaderHashFixturator::new(Predictable).take(2).collect();
        let prev_status = Valid(ChainHead {
            header_seq: 1,
            hash: hashes[0].clone(),
        });
        let incoming_status = Valid(ChainHead {
            header_seq: 1,
            hash: hashes[1].clone(),
        });
        let expected = Forked(ChainFork {
            fork_seq: 1,
            first_header: hashes[0].clone(),
            second_header: hashes[1].clone(),
        });
        assert_eq!(
            add_chain_status(prev_status, incoming_status),
            Some(expected)
        );

        // Empty doesn't overwrite
        let prev_status = Valid(ChainHead {
            header_seq: 1,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(add_chain_status(prev_status, ChainStatus::Empty), None);

        // Same doesn't overwrite
        let prev_status = Valid(ChainHead {
            header_seq: 1,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
        let prev_status = Forked(ChainFork {
            fork_seq: 2,
            first_header: fixt!(HeaderHash),
            second_header: fixt!(HeaderHash),
        });
        assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
        let prev_status = Invalid(ChainHead {
            header_seq: 2,
            hash: fixt!(HeaderHash),
        });
        assert_eq!(add_chain_status(prev_status.clone(), prev_status), None);
    }
}
