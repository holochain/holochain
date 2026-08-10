//! Part of the restore workflow. Walks the records collected in [`super::agent_activity`] backward
//! from the agreed chain head to the genesis, verifying that each record's action hash matches its
//! content, that any attached entry matches its action's declared entry hash, and that the
//! resulting chain is a contiguous, gap-free sequence from the agreed head back to sequence number
//! 0, which is in a genesis `Dna` action.

use std::collections::HashMap;

use holo_hash::{ActionHash, EntryHash};
use holochain_zome_types::prelude::{ActionType, Record, RecordEntry};

/// The result of walking the collected records backward from the agreed chain head.
#[derive(Debug)]
pub(super) enum ReconstructionOutcome {
    /// Every action from the agreed head back to the genesis was resolved.
    /// Holds the chain ordered from genesis to head, ready to be written to the per-DNA database.
    Complete(Vec<Record>),
    /// The collected records do not cover the full chain, this could be due to a `prev_action` link
    /// not being able to be resolved, the sequence numbers not being contiguous down to 0, or that
    /// the terminal action was not a genesis `Dna` action. A fresh acquisition attempt is needed.
    Incomplete,
}

/// Walks records backward from the head's hash, following each action's prev_action link all the
/// way back to genesis' Dna action.
///
/// Any records found to be invalid are discarded before walking back. This could be records whose
/// hash does not match a hash of their content or the attached entry does not match the action's
/// entry hash. Records that are not reachable from the head's hash, such as an abandoned fork
/// branch, are also excluded from the result even though they were present in records. The walk
/// requires strictly contiguous sequence numbers from `head_seq` back to 0 and a genesis `Dna`
/// action at the end, so a peer that agreed on a bogus head cannot pass off a shorter, unrelated
/// chain as the agreed one.
pub(super) fn reconstruct_chain(
    records: Vec<Record>,
    head_seq: u32,
    head_hash: &ActionHash,
) -> ReconstructionOutcome {
    let mut chain = Vec::with_capacity(records.len());

    let mut by_hash: HashMap<ActionHash, Record> = records
        .into_iter()
        .filter(|record| record.action_hashed().verify_hash_sync().is_ok())
        .filter(entry_matches_declared_hash)
        .map(|record| (record.action_address().clone(), record))
        .collect();

    let mut hash = head_hash.clone();

    for expected_seq in (0..=head_seq).rev() {
        let Some(record) = by_hash.remove(&hash) else {
            return ReconstructionOutcome::Incomplete;
        };
        if record.action().action_seq() != expected_seq {
            return ReconstructionOutcome::Incomplete;
        }

        if expected_seq == 0 {
            // The genesis action must be a Dna action with no dangling prev_action.
            if record.action().action_type() != ActionType::Dna
                || record.action().prev_action().is_some()
            {
                return ReconstructionOutcome::Incomplete;
            }
        } else {
            let Some(prev) = record.action().prev_action() else {
                return ReconstructionOutcome::Incomplete;
            };
            hash = prev.clone();
        }

        chain.push(record);
    }

    chain.reverse();
    ReconstructionOutcome::Complete(chain)
}

/// Does the record's action hash match the content, or is there no hash as the record is private.
fn entry_matches_declared_hash(record: &Record) -> bool {
    let Some(declared_hash) = record.action().entry_hash() else {
        return true;
    };
    match record.entry() {
        RecordEntry::Present(entry) => EntryHash::with_data_sync(entry) == *declared_hash,
        RecordEntry::Hidden => true,
        RecordEntry::NA | RecordEntry::NotStored => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, DnaHashFixturator};
    use holochain_zome_types::prelude::*;

    fn dna_action(agent: &AgentPubKey) -> Action {
        Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(0),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::Dna(DnaData {
                dna_hash: fixt!(DnaHash),
            }),
        }
    }

    fn linked_action(agent: &AgentPubKey, seq: u32, prev_action: ActionHash) -> Action {
        Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(seq as i64 * 1000),
                action_seq: seq,
                prev_action: Some(prev_action),
            },
            data: ActionData::InitZomesComplete(InitZomesCompleteData {}),
        }
    }

    fn make_record(action: Action) -> Record {
        let action_hashed = ActionHashed::from_content_sync(action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        Record::new(signed, RecordEntry::NA)
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

        let outcome = reconstruct_chain(records, 3, &head_hash);
        let ReconstructionOutcome::Complete(chain) = outcome else {
            panic!("expected Complete");
        };
        assert_eq!(chain.len(), 4);
        for (i, record) in chain.iter().enumerate() {
            assert_eq!(record.action().action_seq(), i as u32);
        }
        assert!(matches!(chain[0].action().data, ActionData::Dna(_)));
    }

    #[test]
    fn gap_in_chain_returns_incomplete() {
        let agent = fixt!(AgentPubKey);
        let mut records = build_chain(&agent, 4);
        let head_hash = records.last().unwrap().action_address().clone();
        // Remove the record at seq 1, breaking the link between seq 2 and the genesis Dna action.
        records.remove(1);

        let outcome = reconstruct_chain(records, 3, &head_hash);
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

        let outcome = reconstruct_chain(records, 2, &head_hash);
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

        let outcome = reconstruct_chain(records, 3, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }

    #[test]
    fn mismatched_head_seq_returns_incomplete() {
        let agent = fixt!(AgentPubKey);
        let records = build_chain(&agent, 4);
        let head_hash = records.last().unwrap().action_address().clone();

        let outcome = reconstruct_chain(records, 2, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }

    #[test]
    fn non_dna_genesis_returns_incomplete() {
        let agent = fixt!(AgentPubKey);
        let bogus_genesis = linked_action(&agent, 0, fixt!(ActionHash));
        let mut action = bogus_genesis;
        action.header.prev_action = None;
        let record = make_record(action);
        let head_hash = record.action_address().clone();

        let outcome = reconstruct_chain(vec![record], 0, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }

    #[test]
    fn mismatched_entry_hash_is_excluded() {
        let agent = fixt!(AgentPubKey);
        let dna = make_record(dna_action(&agent));
        let real_entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(vec![1; 8]),
            ),
        ));
        let declared_hash = EntryHash::with_data_sync(&real_entry);
        let create_action = Action {
            header: ActionHeader {
                author: agent.clone(),
                timestamp: Timestamp::from_micros(1000),
                action_seq: 1,
                prev_action: Some(dna.action_address().clone()),
            },
            data: ActionData::Create(CreateData {
                entry_type: EntryType::App(AppEntryDef::new(
                    0.into(),
                    0.into(),
                    EntryVisibility::Public,
                )),
                entry_hash: declared_hash,
            }),
        };
        let action_hashed = ActionHashed::from_content_sync(create_action);
        let signed = SignedActionHashed::with_presigned(action_hashed, fixt!(Signature));
        let tampered_entry = Entry::App(AppEntryBytes(
            holochain_serialized_bytes::SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(vec![2; 8]),
            ),
        ));
        let create = Record::new(signed, RecordEntry::Present(tampered_entry));
        let head_hash = create.action_address().clone();

        let outcome = reconstruct_chain(vec![dna, create], 1, &head_hash);
        assert!(matches!(outcome, ReconstructionOutcome::Incomplete));
    }
}
