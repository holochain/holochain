use arbitrary::Arbitrary;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_zome_types::Create;
use holochain_zome_types::Dna;
use std::ops::Range;

use arbitrary::Unstructured;
use holo_hash::ActionHash;
use holochain_zome_types::Action;
use holochain_zome_types::RegisterAgentActivity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChainItem {
    pub action_seq: u32,
    pub hash: ActionHash,
    pub prev_action: Option<ActionHash>,
}

/// Create a hash from a u8.
fn hash(i: &[u8]) -> Vec<u8> {
    let mut i = i.iter().copied().take(36).collect::<Vec<_>>();
    let num_needed = 36 - i.len();
    i.extend(std::iter::repeat(0).take(num_needed));
    i
}

pub fn action_hash(i: &[u8]) -> ActionHash {
    ActionHash::from_raw_36(hash(i))
}

pub fn agent_hash(i: &[u8]) -> AgentPubKey {
    AgentPubKey::from_raw_36(hash(i))
}

pub fn entry_hash(i: &[u8]) -> EntryHash {
    EntryHash::from_raw_36(hash(i))
}

/// Create a chain per agent
pub fn agent_chain(ranges: &[(u8, Range<u8>)]) -> Vec<(AgentPubKey, Vec<ChainItem>)> {
    ranges
        .iter()
        .map(|(a, range)| (agent_hash(&[*a]), chain(range.clone())))
        .collect()
}

/// Create a chain from a range where the first chain items
/// previous hash == that items hash.
pub fn chain(range: Range<u8>) -> Vec<ChainItem> {
    range
        .map(|i| {
            let prev = i.checked_sub(1).map(|i_sub_1| action_hash(&[i_sub_1]));
            ChainItem {
                action_seq: i as u32,
                hash: action_hash(&[i]),
                prev_action: prev,
            }
        })
        .rev()
        .collect()
}

/// Create a set of chains with forks where the first range
/// is the chain that all following ranges fork from.
pub fn forked_chain(ranges: &[Range<u8>]) -> Vec<ChainItem> {
    let mut out = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let r = range
            .clone()
            .enumerate()
            .map(|(j, n)| {
                if j == 0 || i == 0 {
                    let prev = n.checked_sub(1).map(|n_sub_1| action_hash(&[n_sub_1]));
                    ChainItem {
                        action_seq: n as u32,
                        hash: action_hash(&[n as u8]),
                        prev_action: prev,
                    }
                } else {
                    let prev = n
                        .checked_sub(1)
                        .map(|n_sub_1| action_hash(&[n_sub_1, i as u8]));
                    ChainItem {
                        action_seq: n as u32,
                        hash: action_hash(&[n, i as u8]),
                        prev_action: prev,
                    }
                }
            })
            .rev();
        out.extend(r);
    }
    out.sort_unstable_by_key(|s| s.action_seq);
    out.reverse();
    out
}

/// Build a chain with gaps in it. Each range will make a chain even if there
/// are gaps.
pub fn gap_chain(ranges: &[Range<u8>]) -> Vec<ChainItem> {
    let min = ranges.iter().map(|r| r.start).min().unwrap();
    let max = ranges.iter().map(|r| r.end).max().unwrap();
    chain(min..max)
        .into_iter()
        .filter(|i| ranges.iter().any(|r| r.contains(&(i.action_seq as u8))))
        .collect()
}

pub fn chain_to_ops(chain: Vec<ChainItem>) -> Vec<RegisterAgentActivity> {
    let mut u = Unstructured::new(&holochain_zome_types::NOISE);
    chain
        .into_iter()
        .map(
            |ChainItem {
                 action_seq,
                 hash,
                 prev_action,
             }| {
                let mut op = RegisterAgentActivity::arbitrary(&mut u).unwrap();
                match (action_seq, prev_action) {
                    (0, _) => {
                        let dna = Dna::arbitrary(&mut u).unwrap();
                        op.action.hashed.content = Action::Dna(dna);
                        op.action.hashed.hash = hash;
                    }
                    (action_seq, Some(prev_action)) => {
                        let mut create = Create::arbitrary(&mut u).unwrap();
                        create.action_seq = action_seq;
                        create.prev_action = prev_action;
                        op.action.hashed.content = Action::Create(create);
                        op.action.hashed.hash = hash;
                    }
                    _ => unreachable!(),
                }
                op
            },
        )
        .collect()
}
