//! Part of the restore workflow. Walks the records collected in [`super::agent_activity`] backward
//! from the agreed chain head to the genesis, verifying that each record's action hash matches its
//! content before trusting its `prev_action` link.

use std::collections::HashMap;

use holo_hash::ActionHash;
use holochain_zome_types::prelude::Record;

/// The result of walking the collected records backward from the agreed chain head.
#[derive(Debug)]
pub(super) enum ReconstructionOutcome {
    /// Every action from the agreed head back to the genesis was resolved.
    /// Holds the chain ordered from genesis to head, ready to be written to the per-DNA database.
    Complete(Vec<Record>),
    /// The walk could not resolve a `prev_action` link before reaching genesis, meaning the
    /// collected records do not cover the full chain. A fresh acquisition attempt is needed.
    Incomplete,
}

/// Walks records backward from the head's hash, following each action's prev_action link all the
/// way back to genesis' Dna action.
///
/// Records whose declared hash does not match a hash recomputed from their content are discarded
/// before the walk begins, so a tampered `prev_action` label cannot be trusted. Records that are
/// not reachable from the head's hash, such as an abandoned fork branch, are excluded from the
/// result even though they were present in records.
pub(super) fn reconstruct_chain(
    records: Vec<Record>,
    head_hash: &ActionHash,
) -> ReconstructionOutcome {
    let mut by_hash: HashMap<ActionHash, Record> = records
        .into_iter()
        .filter(|record| record.action_hashed().verify_hash_sync().is_ok())
        .map(|record| (record.action_address().clone(), record))
        .collect();

    let mut chain = Vec::new();
    let mut next_hash = Some(head_hash.clone());

    while let Some(hash) = next_hash {
        let Some(record) = by_hash.remove(&hash) else {
            return ReconstructionOutcome::Incomplete;
        };
        next_hash = record.action().prev_action().cloned();
        chain.push(record);
    }

    chain.reverse();
    ReconstructionOutcome::Complete(chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, DnaHashFixturator};
    use holochain_zome_types::prelude::*;

    fn dna_action(agent: &AgentPubKey) -> Action {
        Action::Dna(Dna {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(0),
            hash: fixt!(DnaHash),
        })
    }

    fn linked_action(agent: &AgentPubKey, seq: u32, prev_action: ActionHash) -> Action {
        Action::InitZomesComplete(InitZomesComplete {
            author: agent.clone(),
            timestamp: Timestamp::from_micros(seq as i64 * 1000),
            action_seq: seq,
            prev_action,
        })
    }

    fn make_record(action: Action) -> Record {
        let action_hashed = ActionHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        Record::new(signed, None)
    }

    /// Builds a chain of `len` records (including the genesis `Dna` action) with correct
    /// seq/prev_action linkage, in ascending order.
    fn build_chain(agent: &AgentPubKey, len: u32) -> Vec<Record> {
        let mut records = Vec::new();
        let mut prev_hash = None;
        for seq in 0..len {
            let action = match prev_hash {
                None => dna_action(agent),
                Some(prev) => linked_action(agent, seq, prev),
            };
            let record = make_record(action);
            prev_hash = Some(record.action_address().clone());
            records.push(record);
        }
        records
    }

    #[test]
    fn full_chain_reconstructs_in_order() {
        let agent = fixt!(AgentPubKey);
        let records = build_chain(&agent, 4);
        let head_hash = records.last().unwrap().action_address().clone();

        let outcome = reconstruct_chain(records, &head_hash);
        let ReconstructionOutcome::Complete(chain) = outcome else {
            panic!("expected Complete");
        };
        assert_eq!(chain.len(), 4);
        for (i, record) in chain.iter().enumerate() {
            assert_eq!(record.action().action_seq(), i as u32);
        }
        assert!(matches!(chain[0].action(), Action::Dna(_)));
    }

    #[test]
    fn gap_in_chain_returns_incomplete() {
        let agent = fixt!(AgentPubKey);
        let mut records = build_chain(&agent, 4);
        let head_hash = records.last().unwrap().action_address().clone();
        // Remove the record at seq 1, breaking the link between seq 2 and the genesis Dna action.
        records.remove(1);

        let outcome = reconstruct_chain(records, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }

    #[test]
    fn unreferenced_fork_record_is_excluded() {
        let agent = fixt!(AgentPubKey);
        let mut records = build_chain(&agent, 3);
        let head_hash = records.last().unwrap().action_address().clone();

        // A fork off seq 1 that nothing in the main chain references.
        let fork_prev = records[1].action_address().clone();
        let fork_record = make_record(linked_action(&agent, 2, fork_prev));
        records.push(fork_record);

        let outcome = reconstruct_chain(records, &head_hash);
        let ReconstructionOutcome::Complete(chain) = outcome else {
            panic!("expected Complete");
        };
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn tampered_action_hash_is_excluded_and_breaks_the_walk() {
        let agent = fixt!(AgentPubKey);
        let mut records = build_chain(&agent, 4);
        let head_hash = records.last().unwrap().action_address().clone();
        // Tamper with the stored hash of seq 1 so it no longer matches its content. The record
        // becomes unreachable under its true hash, which seq 2's `prev_action` still points to.
        records[1].signed_action.hashed.hash = fixt!(ActionHash);

        let outcome = reconstruct_chain(records, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }
}
