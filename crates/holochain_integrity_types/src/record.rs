//! Defines a Record, the basic unit of Holochain data.

use crate::entry_def::EntryVisibility;
use crate::signature::Signature;
use crate::Entry;
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
