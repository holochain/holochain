//! Contrafact Facts for types in this crate

use crate::prelude::*;
use contrafact::*;

/// Action is of a given type
pub fn is_of_type(action_type: ActionType) -> impl Fact<'static, Action> {
    facts![brute("action is of type", move |h: &Action| h
        .action_type()
        == action_type)]
}

/// Action is one which brings a new Entry into being
pub fn is_new_entry_action<'a>() -> impl Fact<'a, Action> {
    let et_fact = brute("is NewEntryAction", move |et: &EntryType| {
        matches!(et, EntryType::App(_))
    });

    facts![
        // Must ensure entry type exists, because if not, the prism
        // fact will not be checked
        brute("has entry type", |a: &Action| a.entry_type().is_some()),
        prism(
            "entry type",
            |a: &mut Action| { a.entry_data_mut().map(|(_, et)| et) },
            et_fact
        )
    ]
}

/// Action is one which has no Entry associated
pub fn is_not_entry_action<'a>() -> impl Fact<'a, Action> {
    facts![brute("is not NewEntryAction", move |a: &Action| a
        .entry_type()
        .is_none())]
}

/// A rough check that a sequence of Actions constitutes a valid source chain
/// - First action must be Dna
/// - Each subsequent action's prev_action hash must match the previous action
/// - The seq number must be increasing by 1, from 0
///
/// Notably, this does NOT check the following:
/// xxx Genesis actions in the proper place
/// xxx Genesis actions in the *wrong* place
///
/// TODO: It would be more readable/composable to break this into several parts:
/// - constrain action types based on position
/// - constrain seq num
/// - constrain prev_hashes
/// ...but, this does it all in one Fact
pub fn valid_chain<'a>(len: usize, author: AgentPubKey) -> impl Fact<'a, Vec<Action>> {
    vec_of_length(len, valid_chain_action(author))
}

/// A valid action in a chain. Stateful, builds upon the previous action.
pub fn valid_chain_action<'a>(author: AgentPubKey) -> impl Fact<'a, Action> {
    lambda(
        "valid_chain_action",
        ValidChainFactState::default(),
        move |g, s, mut action: Action| {
            while matches!(action, Action::CloseChain(_) | Action::OpenChain(_)) {
                action = g.arbitrary(|| {
                    "valid_chain_action cannot handle CloseChain or OpenChain".to_string()
                })?;
            }
            match (s.hash.as_ref(), action.prev_action_mut()) {
                (Some(stored), Some(prev)) => {
                    let p = prev.clone();
                    g.set(prev, stored, || {
                        format!("Hashes don't match: {} != {}", p, stored)
                    })?;
                }
                (None, None) => {}
                (Some(_), None) => {
                    action = brute(
                        format!(
                            "Found Dna in position other than beginning of the chain. Hash: {}",
                            ActionHash::with_data_sync(&action)
                        ),
                        |a: &Action| a.action_type() != ActionType::Dna,
                    )
                    .mutate(g, action)?;
                }
                (None, Some(_)) => {
                    let err = || {
                        format!(
                            "First action must be of type Dna, but instead got type {:?}",
                            action.action_type()
                        )
                    };
                    action = Action::Dna(g.arbitrary(err)?);
                }
            };

            match (s.seq, action.action_seq_mut()) {
                (0, None) => {}
                (stored, Some(seq)) if stored > 0 => {
                    g.set(seq, &stored, || "Seq must be 1 more than the last")?
                }
                _ => {
                    return Err(MutationError::User(format!(
                    "ValidChainFact: Action should already be set properly. action={:?}, fact={:?}",
                    action, s
                )))
                }
            }

            g.set(action.author_mut(), &author, || "Author must be the same")?;

            s.hash = Some(ActionHash::with_data_sync(&action));
            s.seq += 1;

            Ok(action)
        },
    )
}

#[derive(Debug, Clone, Default)]
struct ValidChainFactState {
    hash: Option<ActionHash>,
    seq: u32,
}

/// All the mutable references for an Action
#[allow(missing_docs)]
pub trait ActionRefMut {
    fn author_mut(&mut self) -> &mut AgentPubKey;
    fn action_seq_mut(&mut self) -> Option<&mut u32>;
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash>;
    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)>;
    fn timestamp_mut(&mut self) -> &mut Timestamp;
}

/// Some necessary extra mutators for lenses/prisms over Actions
impl ActionRefMut for Action {
    /// returns a mutable reference to the author
    fn author_mut(&mut self) -> &mut AgentPubKey {
        match *self {
            Self::Dna(Dna { ref mut author, .. })
            | Self::AgentValidationPkg(AgentValidationPkg { ref mut author, .. })
            | Self::InitZomesComplete(InitZomesComplete { ref mut author, .. })
            | Self::CreateLink(CreateLink { ref mut author, .. })
            | Self::DeleteLink(DeleteLink { ref mut author, .. })
            | Self::Delete(Delete { ref mut author, .. })
            | Self::CloseChain(CloseChain { ref mut author, .. })
            | Self::OpenChain(OpenChain { ref mut author, .. })
            | Self::Create(Create { ref mut author, .. })
            | Self::Update(Update { ref mut author, .. }) => author,
        }
    }

    /// returns a mutable reference to the timestamp
    fn timestamp_mut(&mut self) -> &mut Timestamp {
        match *self {
            Self::Dna(Dna {
                ref mut timestamp, ..
            })
            | Self::AgentValidationPkg(AgentValidationPkg {
                ref mut timestamp, ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                ref mut timestamp, ..
            })
            | Self::CreateLink(CreateLink {
                ref mut timestamp, ..
            })
            | Self::DeleteLink(DeleteLink {
                ref mut timestamp, ..
            })
            | Self::Delete(Delete {
                ref mut timestamp, ..
            })
            | Self::CloseChain(CloseChain {
                ref mut timestamp, ..
            })
            | Self::OpenChain(OpenChain {
                ref mut timestamp, ..
            })
            | Self::Create(Create {
                ref mut timestamp, ..
            })
            | Self::Update(Update {
                ref mut timestamp, ..
            }) => timestamp,
        }
    }

    /// returns a mutable reference to the sequence ordinal of this action
    fn action_seq_mut(&mut self) -> Option<&mut u32> {
        match *self {
            // Dna is always 0
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut action_seq, ..
            })
            | Self::InitZomesComplete(InitZomesComplete {
                ref mut action_seq, ..
            })
            | Self::CreateLink(CreateLink {
                ref mut action_seq, ..
            })
            | Self::DeleteLink(DeleteLink {
                ref mut action_seq, ..
            })
            | Self::Delete(Delete {
                ref mut action_seq, ..
            })
            | Self::CloseChain(CloseChain {
                ref mut action_seq, ..
            })
            | Self::OpenChain(OpenChain {
                ref mut action_seq, ..
            })
            | Self::Create(Create {
                ref mut action_seq, ..
            })
            | Self::Update(Update {
                ref mut action_seq, ..
            }) => Some(action_seq),
        }
    }

    /// returns the previous action except for the DNA action which doesn't have a previous
    fn prev_action_mut(&mut self) -> Option<&mut ActionHash> {
        match self {
            Self::Dna(Dna { .. }) => None,
            Self::AgentValidationPkg(AgentValidationPkg {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::InitZomesComplete(InitZomesComplete {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::CreateLink(CreateLink {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::DeleteLink(DeleteLink {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Delete(Delete {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::CloseChain(CloseChain {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::OpenChain(OpenChain {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Create(Create {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
            Self::Update(Update {
                ref mut prev_action,
                ..
            }) => Some(prev_action),
        }
    }

    fn entry_data_mut(&mut self) -> Option<(&mut EntryHash, &mut EntryType)> {
        match self {
            Self::Create(Create {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            Self::Update(Update {
                ref mut entry_hash,
                ref mut entry_type,
                ..
            }) => Some((entry_hash, entry_type)),
            _ => None,
        }
    }
}

type Pair = (Action, RecordEntry);

/// Fact: Given a pair of an action and optional Entry:
/// - If the action references an Entry,
///     - the Entry will exist and be of the appropriate hash,
///     - and the entry types will match
/// - If the action does not reference an Entry, the entry will be None
//
// TODO: this Fact is useless until we can write "traversals" in addition to lenses and prisms,
// because we cannot in general use a lens to extract a `&mut (Action, Option<Entry>)`
// from any type, and instead need to operate on a `(&mut Action, &mut Option<Entry>)`.
// (A Traversal is like a lens that can focus on more than one thing at once.)
// Alternatively, this might be an argument for making contrafact work with immutable values
// instead of mutable references.
//
// At least we can use this as a reference to write the same logic for DhtOp and Record,
// which require the same sort of checks.

pub fn action_and_entry_match<'a>(must_be_public: bool) -> impl Fact<'a, Pair> {
    facts![
        brute(
            "Action type matches Entry existence, and is public if exists",
            move |(action, entry): &Pair| {
                let data = action.entry_data();
                match (data, entry) {
                    (Some((_entry_hash, entry_type)), RecordEntry::Present(_)) => {
                        // Ensure that entries are public
                        !must_be_public || entry_type.visibility().is_public()
                    }
                    (None, RecordEntry::Present(_)) => false,
                    (None, _) => true,
                    _ => false,
                }
            }
        ),
        lambda_unit(
            "If there is entry data, the action must point to it",
            |g, pair: Pair| {
                if let Some(entry) = pair.1.as_option() {
                    // NOTE: this could be a `lens` if the previous check were short-circuiting,
                    // but it is possible that this check will run even if the previous check fails,
                    // so use a prism instead.
                    prism(
                        "action's entry hash",
                        |(action, _): &mut Pair| action.entry_data_mut().map(|(hash, _)| hash),
                        eq(EntryHash::with_data_sync(entry)),
                    )
                    .mutate(g, pair)
                } else {
                    Ok(pair)
                }
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts;
    use holo_hash::fixt::AgentPubKeyFixturator;

    proptest::proptest! {
        #[test]
        fn test_action_and_entry_match(seed: u64) {
            let ns = noise(Some(seed), 100_000);
            let mut gg = unstructured(&ns).into();
            let g = &mut gg;

            let e = brute("Is App entry", |e| matches!(e, Entry::App(_))).build(g);
            let a0 = facts::is_not_entry_action().build(g);
            let mut a1 = facts::is_new_entry_action().build(g);
            *a1.entry_data_mut().unwrap().0 = EntryHash::with_data_sync(&e);
            let a1 = a1;

            let pair1: Pair = (a0.clone(), RecordEntry::NA);
            let pair2: Pair = (a0.clone(), RecordEntry::Present(e.clone()));
            let pair3: Pair = (a1.clone(), RecordEntry::NA);
            let pair4: Pair = (a1.clone(), RecordEntry::Present(e.clone()));

            // dbg!(&a0, &a1, &e);

            let fact = action_and_entry_match(false);

            fact.clone().check(&pair1).unwrap();
            assert!(fact.clone().check(&pair2).is_err());
            assert!(fact.clone().check(&pair3).is_err());
            fact.clone().check(&pair4).unwrap();
        }
    }

    #[test]
    fn test_valid_chain_fact() {
        let mut g = unstructured_noise().into();
        let author = ::fixt::fixt!(AgentPubKey);
        let fact = valid_chain(5, author.clone());
        let chain = fact.clone().build(&mut g);
        fact.check(&chain).unwrap();

        let hashes: Vec<_> = chain.iter().map(ActionHash::with_data_sync).collect();
        let backlinks: Vec<_> = chain
            .iter()
            .filter_map(|h| h.prev_action())
            .cloned()
            .collect();
        let action_seqs: Vec<_> = chain.iter().map(|h| h.action_seq()).collect();

        // Ensure that the backlinks line up with the actual hashes
        assert_eq!(hashes[0..chain.len() - 1], backlinks[..]);
        // Ensure that the action seqs form a sequence
        assert_eq!(action_seqs, vec![0, 1, 2, 3, 4]);
    }
}
