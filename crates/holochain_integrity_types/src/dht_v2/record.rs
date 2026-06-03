//! The v2 [`Record`] type (transitional `dht_v2` location; promoted to the
//! canonical `record` module in the legacy-deletion phase).

use super::Action;
use crate::record::{RecordEntry, SignedHashed};
use crate::Entry;
use holo_hash::ActionHash;
use holochain_serialized_bytes::prelude::*;

/// A chain record: a signed v2 action plus its entry, if the action has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct Record {
    /// The signed, hashed v2 action for this record.
    pub signed_action: SignedHashed<Action>,
    /// The entry associated with the action, or why it is absent.
    pub entry: RecordEntry<Entry>,
}

impl Record {
    /// Construct a record from a signed action and its entry slot.
    pub fn new(signed_action: SignedHashed<Action>, entry: RecordEntry<Entry>) -> Self {
        Self {
            signed_action,
            entry,
        }
    }

    /// The v2 action content.
    pub fn action(&self) -> &Action {
        &self.signed_action.hashed.content
    }

    /// The action hash of this record.
    pub fn action_address(&self) -> &ActionHash {
        self.signed_action.as_hash()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dht_v2::{Action, ActionData, ActionHeader, CreateData};
    use crate::record::{RecordEntry, SignedHashed};
    use crate::signature::Signature;
    use crate::EntryType;
    use holo_hash::{ActionHash, AgentPubKey, EntryHash, HoloHashed};

    fn sample_signed_action() -> SignedHashed<Action> {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(42),
                action_seq: 3,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data: ActionData::Create(CreateData {
                entry_type: EntryType::AgentPubKey,
                entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            }),
        };
        let hash = ActionHash::from_raw_36(vec![4u8; 36]);
        let hashed = HoloHashed::with_pre_hashed(action, hash);
        SignedHashed::with_presigned(hashed, Signature([0u8; 64]))
    }

    #[test]
    fn record_exposes_action_and_address() {
        let sah = sample_signed_action();
        let expected_hash = sah.as_hash().clone();
        let record = Record::new(sah, RecordEntry::NA);

        assert_eq!(record.action().header.action_seq, 3);
        assert_eq!(record.action_address(), &expected_hash);
        assert_eq!(record.entry, RecordEntry::NA);
    }
}
