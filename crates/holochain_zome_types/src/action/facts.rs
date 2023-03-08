use crate::prelude::*;
use arbitrary::{Arbitrary, Unstructured};
use contrafact::*;
use holo_hash::*;

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
struct ValidChainFact {
    hash: Option<ActionHash>,
    seq: u32,
    author: AgentPubKey,
}

impl Fact<Action> for ValidChainFact {
    fn check(&self, action: &Action) -> Check {
        let action_hash = ActionHash::with_data_sync(action);
        let result = match (action.prev_action(), self.hash.as_ref()) {
            (Some(prev), Some(stored)) => {
                if prev == stored {
                    Check::pass()
                } else {
                    vec![format!("Hashes don't match: {} != {}", prev, stored)].into()
                }
            }
            (None, None) => Check::pass(),
            (None, Some(_)) => vec![format!(
                "Found Dna in position other than beginning of the chain. Hash: {}",
                action_hash
            )]
            .into(),
            (Some(_), None) => vec![format!(
                "First action must be of type Dna, but instead got type {:?}",
                action.action_type()
            )]
            .into(),
        };

        result
    }

    fn mutate(&self, action: &mut Action, u: &mut Unstructured<'static>) {
        if let Some(stored_hash) = self.hash.as_ref() {
            // This is not the first action we've seen
            while action.prev_action().is_none() {
                // Generate arbitrary actions until we get one with a prev action
                *action = Action::arbitrary(u).unwrap();
            }
            // Set the action's prev hash to the one we stored from our previous
            // visit
            *action.prev_action_mut().unwrap() = stored_hash.clone();
            // Also set the seq to the next value (this should only be None
            // iff prev_action is None)
            *action.action_seq_mut().unwrap() = self.seq;
        } else {
            // This is the first action we've seen, so it must be a Dna
            *action = Action::Dna(Dna::arbitrary(u).unwrap());
        }

        *action.author_mut() = self.author.clone();

        // println!(
        //     "{}  =>  {:?}\n",
        //     ActionHash::with_data_sync(action),
        //     action.prev_action()
        // );
    }

    fn advance(&mut self, action: &Action) {
        self.hash = Some(ActionHash::with_data_sync(action));
        self.seq += 1;
    }
}

pub fn is_of_type(action_type: ActionType) -> Facts<'static, Action> {
    facts![brute("action is of type", move |h: &Action| h
        .action_type()
        == action_type)]
}

pub fn is_new_entry_action() -> Facts<'static, Action> {
    facts![or(
        "is NewEntryAction",
        is_of_type(ActionType::Create),
        is_of_type(ActionType::Update)
    )]
}

/// WIP: Fact: The actions form a valid SourceChain
pub fn valid_chain(author: AgentPubKey) -> Facts<'static, Action> {
    facts![ValidChainFact {
        hash: None,
        seq: 0,
        author
    },]
}

/// Fact: The action must be a NewEntryAction
pub fn new_entry_action() -> Facts<'static, Action> {
    facts![brute("Is a NewEntryAction", |h: &Action| {
        matches!(h.action_type(), ActionType::Create | ActionType::Update)
    }),]
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_valid_chain_fact() {
        let mut u = Unstructured::new(&crate::NOISE);
        let author = ::fixt::fixt!(AgentPubKey);

        let chain = build_seq(&mut u, 5, valid_chain(author.clone()));
        check_seq(chain.as_slice(), valid_chain(author)).unwrap();

        let hashes: Vec<_> = chain
            .iter()
            .map(|h| ActionHash::with_data_sync(h))
            .collect();
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
