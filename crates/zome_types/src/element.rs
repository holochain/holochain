//! Defines a Element, the basic unit of Holochain data.

use crate::{
    entry_def::EntryVisibility, header::HeaderHashed, signature::Signature, Entry, Header,
};
use holo_hash::HeaderAddress;
use holo_hash::{
    hash_type, HasHash, HashableContent, HashableContentBytes, HeaderHash, HoloHashed,
};
use holochain_serialized_bytes::prelude::*;

/// a chain element which is a triple containing the signature of the header along with the
/// entry if the header type has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The signed header for this element
    signed_header: SignedHeaderHashed,
    /// If there is an entry associated with this header it will be here
    maybe_entry: Option<Entry>,
}

impl Element {
    /// Raw element constructor.  Used only when we know that the values are valid.
    pub fn new(signed_header: SignedHeaderHashed, maybe_entry: Option<Entry>) -> Self {
        Self {
            signed_header,
            maybe_entry,
        }
    }

    /// Break this element into its components
    pub fn into_inner(self) -> (SignedHeaderHashed, Option<Entry>) {
        (self.signed_header, self.maybe_entry)
    }

    /// The inner signed header
    pub fn signed_header(&self) -> &SignedHeaderHashed {
        &self.signed_header
    }

    /// Access the signature portion of this triple.
    pub fn signature(&self) -> &Signature {
        self.signed_header.signature()
    }

    /// Access the header address
    pub fn header_address(&self) -> &HeaderAddress {
        self.signed_header.header_address()
    }

    /// Access the Header portion of this triple.
    pub fn header(&self) -> &Header {
        self.signed_header.header()
    }

    /// Access the HeaderHashed portion.
    pub fn header_hashed(&self) -> &HeaderHashed {
        self.signed_header.header_hashed()
    }

    /// Access the Entry portion of this triple as a ElementEntry,
    /// which includes the context around the presence or absence of the entry.
    pub fn entry(&self) -> ElementEntry {
        let maybe_visibilty = self
            .header()
            .entry_data()
            .map(|(_, entry_type)| entry_type.visibility());
        match (self.maybe_entry.as_ref(), maybe_visibilty) {
            (Some(entry), Some(_)) => ElementEntry::Present(entry),
            (None, Some(EntryVisibility::Private)) => ElementEntry::Hidden,
            (None, None) => ElementEntry::NotApplicable,
            (Some(_), None) => {
                unreachable!("Entry is present for a Header type which has no entry reference")
            }
            (None, Some(EntryVisibility::Public)) => unreachable!("Entry data missing for element"),
        }
    }
}

/// Represents the different ways the entry_address reference within a Header
/// can be intepreted
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementEntry<'a> {
    /// The Header has an entry_address reference, and the Entry is accessible.
    Present(&'a Entry),
    /// The Header has an entry_address reference, but we are in a public
    /// context and the entry is private.
    Hidden,
    /// The Header does not contain an entry_address reference.
    NotApplicable,
}

impl<'a> ElementEntry<'a> {
    /// Provides entry data if it exists.
    ///
    /// Collapses the enum down to the two possibilities of
    /// extant or nonextant Entry data
    pub fn as_option(&'a self) -> Option<&'a Entry> {
        if let ElementEntry::Present(entry) = self {
            Some(entry)
        } else {
            None
        }
    }
}

/// A combination of a Header and its signature.
///
/// Has implementations From and Into its tuple form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
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

// HACK: In this representation, we have to clone the Header and store it twice,
// once in the HeaderHashed, and once in the SignedHeader. The reason is that
// the API currently requires references to both types, and it was easier to
// do a simple clone than to refactor the entire struct and API to remove the
// need for one of those references. We probably SHOULD do that refactor at
// some point.
// FIXME: refactor so that HeaderHashed is not stored, and then remove the
// header_hashed method which returns a reference to HeaderHashed.
// BTW, I tried to think about the possibility of the following, but none were easy:
// - Having a lazily instantiable SignedHeader, so we only have to clone if needed
// - Having HeaderHashed take AsRefs for its arguments, so you can have a
//    HeaderHashed of references instead of values
// FIXME: OR, even better yet, do away with this struct and just use
// HoloHashed<SignedHeader> instead, if possible and expedient
/// The header and the signature that signed it
#[derive(Clone, Debug, PartialEq)]
pub struct SignedHeaderHashed {
    header: HeaderHashed,
    // signed_header: SignedHeader,
    signature: Signature,
}

#[allow(missing_docs)]
impl SignedHeaderHashed {
    /// Unwrap the complete contents of this "Hashed" wrapper.
    pub fn into_inner(self) -> (SignedHeader, HeaderHash) {
        let (header, hash) = self.header.into_inner();
        ((header, self.signature).into(), hash)
    }

    // /// Access the main item stored in this wrapper type.
    // pub fn as_content(&self) -> &SignedHeader {
    //     &self.signed_header
    // }

    /// Access the already-calculated hash stored in this wrapper type.
    pub fn as_hash(&self) -> &HeaderHash {
        self.header.as_hash()
    }

    pub fn with_presigned(header: HeaderHashed, signature: Signature) -> Self {
        Self { header, signature }
    }

    /// Break apart into a HeaderHashed and a Signature
    pub fn into_header_and_signature(self) -> (HeaderHashed, Signature) {
        (self.header, self.signature)
    }

    /// Access the Header Hash.
    pub fn header_address(&self) -> &HeaderAddress {
        self.header.as_hash()
    }

    /// Access the Header portion.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Access the HeaderHashed portion.
    pub fn header_hashed(&self) -> &HeaderHashed {
        &self.header
    }

    /// Access the signature portion.
    pub fn signature(&self) -> &Signature {
        &self.signature
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
        SignedHeaderHashed {
            header: HeaderHashed::with_pre_hashed(header, hash),
            signature,
        }
    }
}

impl From<SignedHeaderHashed> for HoloHashed<SignedHeader> {
    fn from(shh: SignedHeaderHashed) -> HoloHashed<SignedHeader> {
        let (signed_header, hash) = shh.into_inner();
        HoloHashed::with_pre_hashed(signed_header, hash)
    }
}
