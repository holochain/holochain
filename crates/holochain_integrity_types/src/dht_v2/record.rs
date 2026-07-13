//! The [`Record`] type.

use super::Action;
use crate::entry_def::EntryVisibility;
use crate::record::{RecordEntry, SignedHashed};
use crate::signature::Signature;
use crate::Entry;
use holo_hash::{ActionHash, HoloHashed};
use holochain_serialized_bytes::prelude::*;

/// A chain record: a signed action plus its entry, if the action has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct Record {
    /// The signed, hashed action for this record.
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

    /// The action content.
    pub fn action(&self) -> &Action {
        &self.signed_action.hashed.content
    }

    /// The action hash of this record.
    pub fn action_address(&self) -> &ActionHash {
        self.signed_action.as_hash()
    }

    /// The signature over this record's action.
    pub fn signature(&self) -> &Signature {
        self.signed_action.signature()
    }

    /// The hashed action portion of this record's signed action.
    pub fn action_hashed(&self) -> &HoloHashed<Action> {
        &self.signed_action.hashed
    }

    /// The entry portion of this record, including the context around the
    /// presence or absence of the entry.
    pub fn entry(&self) -> &RecordEntry<Entry> {
        &self.entry
    }

    /// The signed, hashed action for this record.
    pub fn signed_action(&self) -> &SignedHashed<Action> {
        &self.signed_action
    }

    /// Breaks this record into its signed-action and entry components.
    pub fn into_inner(self) -> (SignedHashed<Action>, RecordEntry<Entry>) {
        (self.signed_action, self.entry)
    }

    /// If the record contains private entry data, replaces the entry with
    /// [`RecordEntry::Hidden`] so it cannot be leaked, and hands the hidden
    /// entry back separately.
    pub fn privatized(self) -> (Self, Option<Entry>) {
        let (entry, hidden) = if let Some(EntryVisibility::Private) = self
            .action()
            .entry_type()
            .map(|entry_type| entry_type.visibility())
        {
            match self.entry {
                RecordEntry::Present(entry) => (RecordEntry::Hidden, Some(entry)),
                other => (other, None),
            }
        } else {
            (self.entry, None)
        };
        let privatized = Self {
            signed_action: self.signed_action,
            entry,
        };
        (privatized, hidden)
    }

    /// A mutable reference to the action content of this record.
    ///
    /// This bypasses the record's hash and signature guarantees: a mutation
    /// through this reference leaves the hash and signature inconsistent with
    /// the action. Intended only for constructing fixtures in tests.
    #[cfg(feature = "test_utils")]
    pub fn as_action_mut(&mut self) -> &mut Action {
        &mut self.signed_action.hashed.content
    }
}

impl crate::action::ActionSequenceAndHash for Record {
    fn action_seq(&self) -> u32 {
        self.action().action_seq()
    }

    fn address(&self) -> &ActionHash {
        self.action_address()
    }
}

impl crate::action::ActionHashedContainer for Record {
    fn action(&self) -> &Action {
        Record::action(self)
    }

    fn action_hash(&self) -> &ActionHash {
        self.action_address()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dht_v2::{Action, ActionData, ActionHeader, CreateData};
    use crate::record::{RecordEntry, SignedHashed};
    use crate::signature::Signature;
    use crate::{Entry, EntryType};
    use holo_hash::{ActionHash, AgentPubKey, EntryHash, HoloHashed};

    fn sample_signed_action_with_entry_type(entry_type: EntryType) -> SignedHashed<Action> {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(42),
                action_seq: 3,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data: ActionData::Create(CreateData {
                entry_type,
                entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            }),
        };
        let hash = ActionHash::from_raw_36(vec![4u8; 36]);
        let hashed = HoloHashed::with_pre_hashed(action, hash);
        SignedHashed::with_presigned(hashed, Signature([0u8; 64]))
    }

    fn sample_signed_action() -> SignedHashed<Action> {
        sample_signed_action_with_entry_type(EntryType::AgentPubKey)
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

    #[test]
    fn record_serde_roundtrip() {
        let record = Record::new(sample_signed_action(), RecordEntry::NA);
        let bytes = holochain_serialized_bytes::encode(&record).unwrap();
        let decoded: Record = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(decoded, record);
    }

    #[test]
    fn record_signature_signed_action_and_action_hashed_accessors() {
        let sah = sample_signed_action();
        let expected_signature = sah.signature().clone();
        let expected_hashed = sah.hashed.clone();
        let record = Record::new(sah, RecordEntry::NA);

        assert_eq!(record.signature(), &expected_signature);
        assert_eq!(record.signed_action().hashed, expected_hashed);
        assert_eq!(record.action_hashed(), &expected_hashed);
    }

    #[test]
    fn record_entry_accessor_returns_the_entry_slot() {
        let entry = Entry::Agent(AgentPubKey::from_raw_36(vec![5u8; 36]));
        let record = Record::new(sample_signed_action(), RecordEntry::Present(entry.clone()));
        assert_eq!(record.entry(), &RecordEntry::Present(entry));
    }

    #[test]
    fn record_into_inner_returns_signed_action_and_entry() {
        let sah = sample_signed_action();
        let expected_hash = sah.as_hash().clone();
        let record = Record::new(sah, RecordEntry::NA);

        let (signed_action, entry) = record.into_inner();
        assert_eq!(signed_action.as_hash(), &expected_hash);
        assert_eq!(entry, RecordEntry::NA);
    }

    #[test]
    fn record_privatized_hides_a_present_private_entry() {
        let entry = Entry::App(crate::AppEntryBytes(SerializedBytes::default()));
        let sah = sample_signed_action_with_entry_type(EntryType::CapClaim);
        let record = Record::new(sah, RecordEntry::Present(entry.clone()));

        let (privatized, hidden) = record.privatized();
        assert_eq!(privatized.entry, RecordEntry::Hidden);
        assert_eq!(hidden, Some(entry));
    }

    #[test]
    fn record_privatized_leaves_a_public_entry_present() {
        let entry = Entry::Agent(AgentPubKey::from_raw_36(vec![6u8; 36]));
        let sah = sample_signed_action_with_entry_type(EntryType::AgentPubKey);
        let record = Record::new(sah, RecordEntry::Present(entry.clone()));

        let (privatized, hidden) = record.privatized();
        assert_eq!(privatized.entry, RecordEntry::Present(entry));
        assert_eq!(hidden, None);
    }

    #[test]
    fn record_as_action_mut_allows_mutation() {
        let mut record = Record::new(sample_signed_action(), RecordEntry::NA);
        let new_author = AgentPubKey::from_raw_36(vec![7u8; 36]);
        record.as_action_mut().header.author = new_author.clone();
        assert_eq!(record.action().author(), &new_author);
    }
}
