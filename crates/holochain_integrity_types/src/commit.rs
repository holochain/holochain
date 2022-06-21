//! Defines a Commit, the basic unit of Holochain data.

use crate::action::conversions::WrongActionError;
use crate::action::ActionHashed;
use crate::action::CreateLink;
use crate::action::DeleteLink;
use crate::entry_def::EntryVisibility;
use crate::signature::Signature;
use crate::Action;
use crate::Entry;
use holo_hash::ActionHash;
use holo_hash::HashableContent;
use holo_hash::HoloHashOf;
use holo_hash::HoloHashed;
use holochain_serialized_bytes::prelude::*;

/// a chain commit containing the signed action along with the
/// entry if the action type has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Commit {
    /// The signed action for this commit
    pub signed_action: SignedActionHashed,
    /// If there is an entry associated with this action it will be here.
    /// If not, there will be an enum variant explaining the reason.
    pub entry: CommitEntry,
}

/// Represents the different ways the entry_address reference within an action
/// can be intepreted
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum CommitEntry {
    /// The Action has an entry_address reference, and the Entry is accessible.
    Present(Entry),
    /// The Action has an entry_address reference, but we are in a public
    /// context and the entry is private.
    Hidden,
    /// The Action does not contain an entry_address reference.
    NotApplicable,
    /// The Action has an entry but was stored without it.
    /// This can happen when you receive gossip of just an action
    /// when the action type is a [`crate::EntryCreationAction`]
    NotStored,
}

/// The hashed action and the signature that signed it
pub type SignedActionHashed = SignedHashed<Action>;

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

impl Commit {
    /// Mutable reference to the Action content.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the Action is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_action_mut(&mut self) -> &mut Action {
        &mut self.signed_action.hashed.content
    }

    /// Mutable reference to the CommitEntry.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the CommitEntry is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_entry_mut(&mut self) -> &mut CommitEntry {
        &mut self.entry
    }

    /// Raw commit constructor.  Used only when we know that the values are valid.
    pub fn new(signed_action: SignedActionHashed, maybe_entry: Option<Entry>) -> Self {
        let maybe_visibility = signed_action
            .action()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility());
        let entry = match (maybe_entry, maybe_visibility) {
            (Some(entry), Some(_)) => CommitEntry::Present(entry),
            (None, Some(EntryVisibility::Private)) => CommitEntry::Hidden,
            (None, None) => CommitEntry::NotApplicable,
            (Some(_), None) => {
                unreachable!("Entry is present for an action type which has no entry reference")
            }
            (None, Some(EntryVisibility::Public)) => CommitEntry::NotStored,
        };
        Self {
            signed_action,
            entry,
        }
    }

    /// If the Commit contains private entry data, set the CommitEntry
    /// to Hidden so that it cannot be leaked
    pub fn privatized(self) -> Self {
        let entry = if let Some(EntryVisibility::Private) = self
            .signed_action
            .action()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility())
        {
            match self.entry {
                CommitEntry::Present(_) => CommitEntry::Hidden,
                other => other,
            }
        } else {
            self.entry
        };
        Self {
            signed_action: self.signed_action,
            entry,
        }
    }

    /// Break this commit into its components
    pub fn into_inner(self) -> (SignedActionHashed, CommitEntry) {
        (self.signed_action, self.entry)
    }

    /// The inner signed-action
    pub fn signed_action(&self) -> &SignedActionHashed {
        &self.signed_action
    }

    /// Access the signature from this commit's signed action
    pub fn signature(&self) -> &Signature {
        self.signed_action.signature()
    }

    /// Access the action address from this commit's signed action
    pub fn action_address(&self) -> &ActionHash {
        self.signed_action.action_address()
    }

    /// Access the Action from this commit's signed action
    pub fn action(&self) -> &Action {
        self.signed_action.action()
    }

    /// Access the ActionHashed from this commit's signed action portion
    pub fn action_hashed(&self) -> &ActionHashed {
        &self.signed_action.hashed
    }

    /// Access the Entry portion of this commit as a CommitEntry,
    /// which includes the context around the presence or absence of the entry.
    pub fn entry(&self) -> &CommitEntry {
        &self.entry
    }
}

impl CommitEntry {
    /// Provides entry data by reference if it exists
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn as_option(&self) -> Option<&Entry> {
        if let CommitEntry::Present(ref entry) = self {
            Some(entry)
        } else {
            None
        }
    }
    /// Provides entry data as owned value if it exists.
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn into_option(self) -> Option<Entry> {
        if let CommitEntry::Present(entry) = self {
            Some(entry)
        } else {
            None
        }
    }

    /// Provides deserialized app entry if it exists
    ///
    /// same as as_option but handles deserialization
    /// anything other tha CommitEntry::Present returns None
    /// a present entry that fails to deserialize cleanly is an error
    /// a present entry that deserializes cleanly is returned as the provided type A
    pub fn to_app_option<A: TryFrom<SerializedBytes, Error = SerializedBytesError>>(
        &self,
    ) -> Result<Option<A>, SerializedBytesError> {
        match self.as_option() {
            Some(Entry::App(eb)) => Ok(Some(A::try_from(SerializedBytes::from(eb.to_owned()))?)),
            _ => Ok(None),
        }
    }

    /// Provides CapGrantEntry if it exists
    ///
    /// same as as_option but handles cap grants
    /// anything other tha CommitEntry::Present for a Entry::CapGrant returns None
    pub fn to_grant_option(&self) -> Option<crate::entry::CapGrantEntry> {
        match self.as_option() {
            Some(Entry::CapGrant(cap_grant_entry)) => Some(cap_grant_entry.to_owned()),
            _ => None,
        }
    }
}

#[cfg(feature = "test_utils")]
impl<T> SignedHashed<T>
where
    T: HashableContent,
    <T as holo_hash::HashableContent>::HashType: holo_hash::hash_type::HashTypeSync,
{
    /// Create a new signed and hashed content by hashing the content.
    pub fn new(content: T, signature: Signature) -> Self {
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

impl From<Commit> for Option<Entry> {
    fn from(e: Commit) -> Self {
        e.entry.into_option()
    }
}

impl TryFrom<Commit> for CreateLink {
    type Error = WrongActionError;
    fn try_from(value: Commit) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_inner()
            .0
            .into_content()
            .try_into()
    }
}

impl TryFrom<Commit> for DeleteLink {
    type Error = WrongActionError;
    fn try_from(value: Commit) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_inner()
            .0
            .into_content()
            .try_into()
    }
}

#[cfg(feature = "test_utils")]
impl<'a, T> arbitrary::Arbitrary<'a> for SignedHashed<T>
where
    T: HashableContent,
    T: arbitrary::Arbitrary<'a>,
    <T as holo_hash::HashableContent>::HashType: holo_hash::PrimitiveHashType,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            hashed: HoloHashed::<T>::arbitrary(u)?,
            signature: Signature::arbitrary(u)?,
        })
    }
}
