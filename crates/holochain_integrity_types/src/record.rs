//! Defines a Record, the basic unit of Holochain data.

use std::borrow::Borrow;

use crate::action::conversions::WrongActionError;
use crate::action::ActionHashed;
use crate::action::CreateLink;
use crate::action::DeleteLink;
use crate::entry_def::EntryVisibility;
use crate::signature::Signature;
use crate::Entry;
use crate::{Action, ActionHashedContainer, ActionSequenceAndHash};
use holo_hash::ActionHash;
use holo_hash::HasHash;
use holo_hash::HashableContent;
use holo_hash::HoloHashOf;
use holo_hash::HoloHashed;
use holo_hash::PrimitiveHashType;
use holochain_serialized_bytes::prelude::*;

/// a chain record containing the signed action along with the
/// entry if the action type has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct Record<A = SignedActionHashed> {
    /// The signed action for this record
    pub signed_action: A,
    /// If there is an entry associated with this action it will be here.
    /// If not, there will be an enum variant explaining the reason.
    pub entry: RecordEntry<Entry>,
}

impl<A> AsRef<A> for Record<A> {
    fn as_ref(&self) -> &A {
        &self.signed_action
    }
}

impl ActionHashedContainer for Record {
    fn action(&self) -> &Action {
        self.action()
    }

    fn action_hash(&self) -> &ActionHash {
        self.action_address()
    }
}

impl ActionSequenceAndHash for Record {
    fn action_seq(&self) -> u32 {
        self.action().action_seq()
    }

    fn address(&self) -> &ActionHash {
        self.action_address()
    }
}

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
    /// when the action type is a [`crate::EntryCreationAction`],
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

/// The hashed action and the signature that signed it
pub type SignedActionHashed = SignedHashed<Action>;

impl AsRef<SignedActionHashed> for SignedActionHashed {
    fn as_ref(&self) -> &SignedActionHashed {
        self
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize)]
/// Any content that has been hashed and signed.
pub struct SignedHashed<T>
where
    T: HashableContent,
{
    /// The hashed content.
    pub hashed: HoloHashed<T>,
    /// The signature of the content.
    pub signature: Signature,
}

impl Record {
    /// Raw record constructor.  Used only when we know that the values are valid.
    /// NOTE: this will NOT hide private entry data if present!
    pub fn new(signed_action: SignedActionHashed, maybe_entry: Option<Entry>) -> Self {
        let maybe_visibility = signed_action.action().entry_visibility();
        let entry = RecordEntry::new(maybe_visibility, maybe_entry);
        Self {
            signed_action,
            entry,
        }
    }

    /// Access the signature from this record's signed action
    pub fn signature(&self) -> &Signature {
        self.signed_action.signature()
    }

    /// Mutable reference to the Action content.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the Action is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_action_mut(&mut self) -> &mut Action {
        &mut self.signed_action.hashed.content
    }

    /// If the Record contains private entry data, set the RecordEntry
    /// to Hidden so that it cannot be leaked. If the entry was hidden,
    /// return it separately.
    pub fn privatized(self) -> (Self, Option<Entry>) {
        let (entry, hidden) = if let Some(EntryVisibility::Private) = self
            .signed_action
            .action()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility())
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

    /// Access the action address from this record's signed action
    pub fn action_address(&self) -> &ActionHash {
        self.signed_action.action_address()
    }

    /// Access the Action from this record's signed action
    pub fn action(&self) -> &Action {
        self.signed_action.action()
    }

    /// Access the ActionHashed from this record's signed action portion
    pub fn action_hashed(&self) -> &ActionHashed {
        &self.signed_action.hashed
    }

    /// Access the Entry portion of this record as a RecordEntry,
    /// which includes the context around the presence or absence of the entry.
    pub fn entry(&self) -> &RecordEntry {
        &self.entry
    }
}

impl<A> Record<A> {
    /// Mutable reference to the RecordEntry.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the RecordEntry is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_entry_mut(&mut self) -> &mut RecordEntry {
        &mut self.entry
    }

    /// Break this record into its components
    pub fn into_inner(self) -> (A, RecordEntry) {
        (self.signed_action, self.entry)
    }

    /// The inner signed-action
    pub fn signed_action(&self) -> &A {
        &self.signed_action
    }
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

impl SignedActionHashed {
    /// Access the Action Hash.
    pub fn action_address(&self) -> &ActionHash {
        &self.hashed.hash
    }

    /// Access the Action portion.
    pub fn action(&self) -> &Action {
        &self.hashed.content
    }

    /// Create a new SignedActionHashed from a type that implements into `Action` and
    /// has the same hash bytes.
    /// The caller must make sure the hash does not change.
    pub fn raw_from_same_hash<T>(other: SignedHashed<T>) -> Self
    where
        T: Into<Action>,
        T: HashableContent<HashType = holo_hash::hash_type::Action>,
    {
        let SignedHashed {
            hashed: HoloHashed { content, hash },
            signature,
        } = other;
        let action = content.into();
        let hashed = ActionHashed::with_pre_hashed(action, hash);
        Self { hashed, signature }
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

impl From<ActionHashed> for Action {
    fn from(action_hashed: ActionHashed) -> Action {
        action_hashed.into_content()
    }
}

impl From<SignedActionHashed> for Action {
    fn from(signed_action_hashed: SignedActionHashed) -> Action {
        ActionHashed::from(signed_action_hashed).into()
    }
}

impl From<Record> for Option<Entry> {
    fn from(e: Record) -> Self {
        e.entry.into_option()
    }
}

impl TryFrom<Record> for CreateLink {
    type Error = WrongActionError;
    fn try_from(value: Record) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_inner()
            .0
            .into_content()
            .try_into()
    }
}

impl TryFrom<Record> for DeleteLink {
    type Error = WrongActionError;
    fn try_from(value: Record) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_inner()
            .0
            .into_content()
            .try_into()
    }
}
