//! Defines a Record, the basic unit of Holochain data.

use crate::action::Action;
use crate::entry::Entry;
use crate::entry_def::EntryVisibility;
use crate::signature::Signature;
use holo_hash::ActionHash;
use holo_hash::HasHash;
use holo_hash::HashableContent;
use holo_hash::HoloHashOf;
use holo_hash::HoloHashed;
use holo_hash::PrimitiveHashType;
use holochain_serialized_bytes::prelude::*;
use std::borrow::Borrow;

/// Represents the different ways the entry_address reference within an action
/// can be interpreted
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes)]
pub enum RecordEntry<E: Borrow<Entry> = Entry> {
    /// The Action has an entry_address reference, and the Entry is accessible.
    Present(E),
    /// The Action has an entry_address reference, but we are in a public
    /// context and the entry is private.
    Hidden,
    /// The Action does not contain an entry_address reference, so there will
    /// never be an associated Entry.
    NA,
    /// The Action has an entry but was stored without it.
    /// This can happen when you receive gossip of just an action
    /// when the action type creates an entry (a
    /// [`Create`](crate::action::ActionData::Create) or
    /// [`Update`](crate::action::ActionData::Update)),
    /// in particular for certain DhtOps
    NotStored,
}

impl<E: Borrow<Entry>> From<E> for RecordEntry<E> {
    fn from(entry: E) -> Self {
        RecordEntry::Present(entry)
    }
}

impl<E: Borrow<Entry>> RecordEntry<E> {
    /// Constructor based on Action data
    pub fn new(vis: Option<&EntryVisibility>, maybe_entry: Option<E>) -> Self {
        match (maybe_entry, vis) {
            (Some(entry), Some(_)) => RecordEntry::Present(entry),
            (None, Some(EntryVisibility::Private)) => RecordEntry::Hidden,
            (None, None) => RecordEntry::NA,
            (Some(_), None) => {
                // TODO this is a problem case but it is reachable
                unreachable!("Entry is present for an action type which has no entry reference")
            }
            (None, Some(EntryVisibility::Public)) => RecordEntry::NotStored,
        }
    }

    /// Provides entry data by reference if it exists
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn as_option(&self) -> Option<&E> {
        if let RecordEntry::Present(ref entry) = self {
            Some(entry)
        } else {
            None
        }
    }

    /// Provides entry data as owned value if it exists.
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn into_option(self) -> Option<E> {
        if let RecordEntry::Present(entry) = self {
            Some(entry)
        } else {
            None
        }
    }

    /// Provides deserialized app entry if it exists
    ///
    /// same as as_option but handles deserialization
    /// anything other than RecordEntry::Present returns None
    /// a present entry that fails to deserialize cleanly is an error
    /// a present entry that deserializes cleanly is returned as the provided type A
    pub fn to_app_option<A: TryFrom<SerializedBytes, Error = SerializedBytesError>>(
        &self,
    ) -> Result<Option<A>, SerializedBytesError> {
        match self.as_option().map(|e| e.borrow()) {
            Some(Entry::App(eb)) => Ok(Some(A::try_from(SerializedBytes::from(eb.to_owned()))?)),
            _ => Ok(None),
        }
    }

    /// Use a reference to the Entry, if present
    pub fn as_ref<'a>(&'a self) -> RecordEntry<&'a E>
    where
        &'a E: Borrow<Entry>,
    {
        match self {
            RecordEntry::Present(ref e) => RecordEntry::Present(e),
            RecordEntry::Hidden => RecordEntry::Hidden,
            RecordEntry::NA => RecordEntry::NA,
            RecordEntry::NotStored => RecordEntry::NotStored,
        }
    }

    /// Provides CapGrantEntry if it exists
    ///
    /// same as as_option but handles cap grants
    /// anything other tha RecordEntry::Present for a Entry::CapGrant returns None
    pub fn to_grant_option(&self) -> Option<crate::entry::CapGrantEntry> {
        match self.as_option().map(|e| e.borrow()) {
            Some(Entry::CapGrant(cap_grant_entry)) => Some(cap_grant_entry.to_owned()),
            _ => None,
        }
    }

    /// If no entry is available, return Hidden, else return Present
    pub fn or_hidden(entry: Option<E>) -> Self {
        entry.map(Self::Present).unwrap_or(Self::Hidden)
    }

    /// If no entry is available, return NotApplicable, else return Present
    pub fn or_not_applicable(entry: Option<E>) -> Self {
        entry.map(Self::Present).unwrap_or(Self::NA)
    }

    /// If no entry is available, return NotStored, else return Present
    pub fn or_not_stored(entry: Option<E>) -> Self {
        entry.map(Self::Present).unwrap_or(Self::NotStored)
    }
}

/// Alias for record with ref entry
pub type RecordEntryRef<'a> = RecordEntry<&'a Entry>;

/// Any content that has been hashed and signed.
#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
pub struct SignedHashed<T>
where
    T: HashableContent,
{
    /// The hashed content.
    pub hashed: HoloHashed<T>,
    /// The signature of the content.
    pub signature: Signature,
}

#[cfg(feature = "hashing")]
impl<T> SignedHashed<T>
where
    T: HashableContent,
    <T as holo_hash::HashableContent>::HashType: holo_hash::hash_type::HashTypeSync,
{
    /// Create a new signed and hashed content by hashing the content, but without checking
    /// the signature.
    pub fn new_unchecked(content: T, signature: Signature) -> Self {
        let hashed = HoloHashed::from_content_sync(content);
        Self { hashed, signature }
    }
}

impl<T> std::hash::Hash for SignedHashed<T>
where
    T: HashableContent,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.signature.hash(state);
        self.as_hash().hash(state);
    }
}

impl<T> std::cmp::PartialEq for SignedHashed<T>
where
    T: HashableContent,
{
    fn eq(&self, other: &Self) -> bool {
        self.hashed == other.hashed && self.signature == other.signature
    }
}

impl<T> SignedHashed<T>
where
    T: HashableContent,
{
    /// Destructure into a [`HoloHashed`] and [`Signature`].
    pub fn into_inner(self) -> (HoloHashed<T>, Signature) {
        (self.hashed, self.signature)
    }

    /// Access the already-calculated hash stored in this wrapper type.
    pub fn as_hash(&self) -> &HoloHashOf<T> {
        &self.hashed.hash
    }

    /// Create with an existing signature.
    pub fn with_presigned(hashed: HoloHashed<T>, signature: Signature) -> Self {
        Self { hashed, signature }
    }

    /// Access the signature portion.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

impl<C: HashableContent<HashType = T>, T: PrimitiveHashType> HashableContent for SignedHashed<C> {
    type HashType = C::HashType;

    fn hash_type(&self) -> Self::HashType {
        T::new()
    }

    fn hashable_content(&self) -> holo_hash::HashableContentBytes {
        holo_hash::HashableContentBytes::Prehashed39(self.hashed.as_hash().get_raw_39().to_vec())
    }
}

impl<C: HashableContent> HasHash for SignedHashed<C> {
    type HashType = C::HashType;

    fn as_hash(&self) -> &HoloHashOf<C> {
        self.hashed.as_hash()
    }

    fn into_hash(self) -> HoloHashOf<C> {
        self.hashed.into_hash()
    }
}

impl<T> From<SignedHashed<T>> for HoloHashed<T>
where
    T: HashableContent,
{
    fn from(sh: SignedHashed<T>) -> HoloHashed<T> {
        sh.hashed
    }
}

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
    use crate::action::{Action, ActionData, ActionHeader, CreateData, EntryType};
    use crate::entry::AppEntryBytes;
    use crate::record::{RecordEntry, SignedHashed};
    use crate::signature::Signature;
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
        let entry = Entry::App(AppEntryBytes(SerializedBytes::default()));
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
