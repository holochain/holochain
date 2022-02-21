//! Defines a Element, the basic unit of Holochain data.

use crate::entry_def::EntryVisibility;
use crate::header::conversions::WrongHeaderError;
use crate::header::CreateLink;
use crate::header::DeleteLink;
use crate::header::HeaderHashed;
use crate::signature::Signature;
use crate::Entry;
use crate::Header;
use holo_hash::hash_type;
use holo_hash::hash_type::HashTypeSync;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
use holo_hash::HeaderHash;
use holo_hash::HoloHashOf;
use holo_hash::HoloHashed;
use holo_hash::PrimitiveHashType;
use holochain_serialized_bytes::prelude::*;

#[cfg(feature = "test_utils")]
pub mod facts;

/// a chain element containing the signed header along with the
/// entry if the header type has one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Element {
    /// The signed header for this element
    signed_header: SignedHeaderHashed,
    /// If there is an entry associated with this header it will be here.
    /// If not, there will be an enum variant explaining the reason.
    entry: ElementEntry,
}

impl Element {
    /// Mutable reference to the Header content.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the Header is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_header_mut(&mut self) -> &mut Header {
        &mut self.signed_header.hashed.content
    }

    /// Mutable reference to the ElementEntry.
    /// This is useless and dangerous in production usage.
    /// Guaranteed to make hashes and signatures mismatch whatever the ElementEntry is mutated to (at least).
    /// This may be useful for tests that rely heavily on mocked and fixturated data.
    #[cfg(feature = "test_utils")]
    pub fn as_entry_mut(&mut self) -> &mut ElementEntry {
        &mut self.entry
    }

    /// Raw element constructor.  Used only when we know that the values are valid.
    pub fn new(signed_header: SignedHeaderHashed, maybe_entry: Option<Entry>) -> Self {
        let maybe_visibility = signed_header
            .header()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility());
        let entry = match (maybe_entry, maybe_visibility) {
            (Some(entry), Some(_)) => ElementEntry::Present(entry),
            (None, Some(EntryVisibility::Private)) => ElementEntry::Hidden,
            (None, None) => ElementEntry::NotApplicable,
            (Some(_), None) => {
                unreachable!("Entry is present for a Header type which has no entry reference")
            }
            (None, Some(EntryVisibility::Public)) => ElementEntry::NotStored,
        };
        Self {
            signed_header,
            entry,
        }
    }

    /// If the Element contains private entry data, set the ElementEntry
    /// to Hidden so that it cannot be leaked
    pub fn privatized(self) -> Self {
        let entry = if let Some(EntryVisibility::Private) = self
            .signed_header
            .header()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility())
        {
            match self.entry {
                ElementEntry::Present(_) => ElementEntry::Hidden,
                other => other,
            }
        } else {
            self.entry
        };
        Self {
            signed_header: self.signed_header,
            entry,
        }
    }

    /// Break this element into its components
    pub fn into_inner(self) -> (SignedHeaderHashed, ElementEntry) {
        (self.signed_header, self.entry)
    }

    /// The inner signed-header
    pub fn signed_header(&self) -> &SignedHeaderHashed {
        &self.signed_header
    }

    /// Access the signature from this element's signed header
    pub fn signature(&self) -> &Signature {
        self.signed_header.signature()
    }

    /// Access the header address from this element's signed header
    pub fn header_address(&self) -> &HeaderHash {
        self.signed_header.header_address()
    }

    /// Access the Header from this element's signed header
    pub fn header(&self) -> &Header {
        self.signed_header.header()
    }

    /// Access the HeaderHashed from this element's signed header portion
    pub fn header_hashed(&self) -> &HeaderHashed {
        &self.signed_header.hashed
    }

    /// Access the Entry portion of this element as an ElementEntry,
    /// which includes the context around the presence or absence of the entry.
    pub fn entry(&self) -> &ElementEntry {
        &self.entry
    }
}

/// Represents the different ways the entry_address reference within a Header
/// can be intepreted
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum ElementEntry {
    /// The Header has an entry_address reference, and the Entry is accessible.
    Present(Entry),
    /// The Header has an entry_address reference, but we are in a public
    /// context and the entry is private.
    Hidden,
    /// The Header does not contain an entry_address reference.
    NotApplicable,
    /// The Header has an entry but was stored without it.
    /// This can happen when you receive gossip of just a header
    /// when the header type is a [NewEntryHeader]
    NotStored,
}

impl ElementEntry {
    /// Provides entry data by reference if it exists
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn as_option(&self) -> Option<&Entry> {
        if let ElementEntry::Present(ref entry) = self {
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
        if let ElementEntry::Present(entry) = self {
            Some(entry)
        } else {
            None
        }
    }

    /// Provides deserialized app entry if it exists
    ///
    /// same as as_option but handles deserialization
    /// anything other than ElementEntry::Present returns None
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
    /// anything other than ElementEntry::Present for a Entry::CapGrant returns None
    pub fn to_grant_option(&self) -> Option<crate::entry::CapGrantEntry> {
        match self.as_option() {
            Some(Entry::CapGrant(cap_grant_entry)) => Some(cap_grant_entry.to_owned()),
            _ => None,
        }
    }
}

/// A combination of a Header and its signature.
///
/// Has implementations From and Into its tuple form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SignedHeader(pub Header, pub Signature);

impl SignedHeader {
    /// Accessor for the Header
    pub fn header(&self) -> &Header {
        &self.0
    }

    /// Accessor for the Signature
    pub fn signature(&self) -> &Signature {
        &self.1
    }
}

impl HashableContent for SignedHeader {
    type HashType = hash_type::Header;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Header
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            (&self.0)
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

/// The hashed header and the signature that signed it
pub type SignedHeaderHashed = SignedHashed<Header>;

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

impl<T> SignedHashed<T>
where
    T: HashableContent,
    <T as holo_hash::HashableContent>::HashType: HashTypeSync,
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

impl SignedHeaderHashed {
    /// Unwrap the complete contents of this "Hashed" wrapper.
    pub fn into_inner(self) -> (SignedHeader, HeaderHash) {
        (
            (self.hashed.content, self.signature).into(),
            self.hashed.hash,
        )
    }

    /// Break apart into a HeaderHashed and a Signature
    pub fn into_header_and_signature(self) -> (HeaderHashed, Signature) {
        (self.hashed, self.signature)
    }

    /// Access the Header Hash.
    pub fn header_address(&self) -> &HeaderHash {
        &self.hashed.hash
    }

    /// Access the Header portion.
    pub fn header(&self) -> &Header {
        &self.hashed.content
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

impl From<HeaderHashed> for Header {
    fn from(header_hashed: HeaderHashed) -> Header {
        header_hashed.into_content()
    }
}

impl From<SignedHeaderHashed> for Header {
    fn from(signed_header_hashed: SignedHeaderHashed) -> Header {
        HeaderHashed::from(signed_header_hashed).into()
    }
}

impl From<(Header, Signature)> for SignedHeader {
    fn from((h, s): (Header, Signature)) -> Self {
        Self(h, s)
    }
}

impl From<SignedHeader> for (Header, Signature) {
    fn from(s: SignedHeader) -> Self {
        (s.0, s.1)
    }
}

impl From<HoloHashed<SignedHeader>> for SignedHeaderHashed {
    fn from(hashed: HoloHashed<SignedHeader>) -> SignedHeaderHashed {
        let (signed_header, hash) = hashed.into_inner();
        let SignedHeader(header, signature) = signed_header;
        let hashed = HeaderHashed::with_pre_hashed(header, hash);
        SignedHeaderHashed { hashed, signature }
    }
}

impl From<SignedHeaderHashed> for HoloHashed<SignedHeader> {
    fn from(shh: SignedHeaderHashed) -> HoloHashed<SignedHeader> {
        let (signed_header, hash) = shh.into_inner();
        HoloHashed::with_pre_hashed(signed_header, hash)
    }
}

impl From<SignedHeaderHashed> for SignedHeader {
    fn from(shh: SignedHeaderHashed) -> SignedHeader {
        let (signed_header, _) = shh.into_inner();
        signed_header
    }
}

impl From<Element> for Option<Entry> {
    fn from(e: Element) -> Self {
        e.entry.into_option()
    }
}

impl TryFrom<Element> for CreateLink {
    type Error = WrongHeaderError;
    fn try_from(value: Element) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_header_and_signature()
            .0
            .into_content()
            .try_into()
    }
}

impl TryFrom<Element> for DeleteLink {
    type Error = WrongHeaderError;
    fn try_from(value: Element) -> Result<Self, Self::Error> {
        value
            .into_inner()
            .0
            .into_header_and_signature()
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
    <T as holo_hash::HashableContent>::HashType: PrimitiveHashType,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            hashed: HoloHashed::<T>::arbitrary(u)?,
            signature: Signature::arbitrary(u)?,
        })
    }
}
