use super::*;
use holo_hash::HOLO_HASH_SERIALIZED_LEN;
pub(super) use misc::*;

mod misc;

/// Some keys do not store an array of bytes
/// so can not impl AsRef<[u8]>.
/// This is the key type for those keys to impl into
#[derive(
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    derive_more::Into,
    derive_more::From,
    derive_more::AsRef,
    Clone,
    Debug,
)]
#[as_ref(forward)]
pub struct BytesKey(pub Vec<u8>);

/// The value stored in the links meta db
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct LinkMetaVal {
    /// Hash of the [CreateLink] [Header] that created this link
    pub link_add_hash: HeaderHash,
    /// The [Entry] being linked to
    pub target: EntryHash,
    /// When the link was added
    pub timestamp: Timestamp,
    /// The [ZomePosition] of the zome this link belongs to
    pub zome_id: ZomeId,
    /// A tag used to find this link
    pub tag: LinkTag,
}

/// Key for the LinkMeta database.
///
/// Constructed so that links can be queried by a prefix match
/// on the key.
/// Must provide `tag` and `link_add_hash` for inserts,
/// but both are optional for gets.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum LinkMetaKey<'a> {
    /// Search for all links on a base
    Base(&'a EntryHash),
    /// Search for all links on a base, for a zome
    BaseZome(&'a EntryHash, ZomeId),
    /// Search for all links on a base, for a zome and with a tag
    BaseZomeTag(&'a EntryHash, ZomeId, &'a LinkTag),
    /// This will match only the link created with a certain [CreateLink] hash
    Full(&'a EntryHash, ZomeId, &'a LinkTag, &'a HeaderHash),
}

pub(super) type SysMetaKey = AnyDhtHash;

/// Values of [Header]s stored by the sys meta db
#[derive(Debug, Hash, PartialEq, Eq, Ord, PartialOrd, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {
    /// A header that results in a new entry
    /// Either a [Create] or [Update]
    NewEntry(TimedHeaderHash),
    /// An [Update] [Header]
    Update(TimedHeaderHash),
    /// An [Header::Delete]
    Delete(TimedHeaderHash),
    /// Activity on an agent's public key
    Activity(TimedHeaderHash),
    /// Link remove on link add
    DeleteLink(TimedHeaderHash),
    /// Custom Validation Package
    CustomPackage(HeaderHash),
}

// #[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
/// Subset of headers for the sys meta db
pub(super) enum EntryHeader {
    NewEntry(Header),
    Update(Header),
    Delete(Header),
}

/// To allow partial matching of all chain items on
/// an agents key, a chain sequence position and
/// a specific header we use this enum in a similar way to
/// the [LinkMetaKey]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ChainItemKey {
    /// Match all headers on this agents key
    Agent(AgentPubKey),
    /// Match all headers on this agents key at this sequence number
    AgentSequence(AgentPubKey, u32),
    /// Match a specific header at this key / sequence number
    Full(AgentPubKey, u32, HeaderHash),
}

impl LinkMetaVal {
    /// Turn into a zome friendly type
    pub fn into_link(self) -> holochain_zome_types::link::Link {
        let timestamp: chrono::DateTime<chrono::Utc> = self.timestamp.into();
        holochain_zome_types::link::Link {
            target: self.target,
            timestamp: timestamp.into(),
            tag: self.tag,
        }
    }
}

impl LinkMetaVal {
    /// Create a new Link for the link meta db
    pub fn new(
        link_add_hash: HeaderHash,
        target: EntryHash,
        timestamp: Timestamp,
        zome_id: ZomeId,
        tag: LinkTag,
    ) -> Self {
        Self {
            link_add_hash,
            target,
            timestamp,
            zome_id,
            tag,
        }
    }
}

impl BufKey for BytesKey {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        bytes.into()
    }
}

impl EntryHeader {
    pub(super) fn into_hash(self) -> Result<TimedHeaderHash, SerializedBytesError> {
        let header = match self {
            EntryHeader::NewEntry(h) | EntryHeader::Update(h) | EntryHeader::Delete(h) => h,
        };
        let (header, header_hash): (Header, HeaderHash) =
            HeaderHashed::from_content_sync(header).into();
        Ok(TimedHeaderHash {
            timestamp: header.timestamp().into(),
            header_hash,
        })
    }
}

impl<'a> LinkMetaKey<'a> {
    /// Return the base of this key
    pub fn base(&self) -> &EntryHash {
        use LinkMetaKey::*;
        match self {
            Base(b) | BaseZome(b, _) | BaseZomeTag(b, _, _) | Full(b, _, _, _) => b,
        }
    }
}

impl From<&LinkMetaKey<'_>> for BytesKey {
    fn from(key: &LinkMetaKey<'_>) -> Self {
        use LinkMetaKey::*;
        match key {
            Base(base) => base.as_ref().to_vec(),
            BaseZome(base, zome) => [base.as_ref(), &[u8::from(*zome)]].concat(),
            BaseZomeTag(base, zome, tag) => {
                [base.as_ref(), &[u8::from(*zome)], tag.as_ref()].concat()
            }
            Full(base, zome, tag, link) => [
                base.as_ref(),
                &[u8::from(*zome)],
                tag.as_ref(),
                link.as_ref(),
            ]
            .concat(),
        }
        .into()
    }
}

impl From<LinkMetaKey<'_>> for BytesKey {
    fn from(key: LinkMetaKey<'_>) -> Self {
        (&key).into()
    }
}

impl From<SysMetaVal> for HeaderHash {
    fn from(v: SysMetaVal) -> Self {
        match v {
            SysMetaVal::NewEntry(h)
            | SysMetaVal::Update(h)
            | SysMetaVal::Delete(h)
            | SysMetaVal::DeleteLink(h)
            | SysMetaVal::Activity(h) => h.header_hash,
            SysMetaVal::CustomPackage(h) => h,
        }
    }
}

impl From<NewEntryHeader> for EntryHeader {
    fn from(h: NewEntryHeader) -> Self {
        EntryHeader::NewEntry(h.into())
    }
}

impl From<header::Update> for EntryHeader {
    fn from(h: header::Update) -> Self {
        EntryHeader::Update(Header::Update(h))
    }
}

impl From<header::Delete> for EntryHeader {
    fn from(h: header::Delete) -> Self {
        EntryHeader::Delete(Header::Delete(h))
    }
}

impl<'a> From<(&'a CreateLink, &'a HeaderHash)> for LinkMetaKey<'a> {
    fn from((link_add, hash): (&'a CreateLink, &'a HeaderHash)) -> Self {
        Self::Full(
            &link_add.base_address,
            link_add.zome_id,
            &link_add.tag,
            hash,
        )
    }
}

impl<'a> From<&'a WireLinkMetaKey> for LinkMetaKey<'a> {
    fn from(wire_link_meta_key: &'a WireLinkMetaKey) -> Self {
        match wire_link_meta_key {
            WireLinkMetaKey::Base(base) => Self::Base(base),
            WireLinkMetaKey::BaseZome(base, zome) => Self::BaseZome(base, *zome),
            WireLinkMetaKey::BaseZomeTag(base, zome, tag) => Self::BaseZomeTag(base, *zome, tag),
            WireLinkMetaKey::Full(base, zome, tag, link) => Self::Full(base, *zome, tag, link),
        }
    }
}

impl From<&LinkMetaKey<'_>> for WireLinkMetaKey {
    fn from(key: &LinkMetaKey) -> Self {
        match key.clone() {
            LinkMetaKey::Base(base) => Self::Base(base.clone()),
            LinkMetaKey::BaseZome(base, zome) => Self::BaseZome(base.clone(), zome),
            LinkMetaKey::BaseZomeTag(base, zome, tag) => {
                Self::BaseZomeTag(base.clone(), zome, tag.clone())
            }
            LinkMetaKey::Full(base, zome, tag, link) => {
                Self::Full(base.clone(), zome, tag.clone(), link.clone())
            }
        }
    }
}

impl From<&[u8]> for BytesKey {
    fn from(b: &[u8]) -> Self {
        Self(b.to_owned())
    }
}

impl IntoIterator for &LinkMetaKey<'_> {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        let b: BytesKey = self.into();
        b.0.into_iter()
    }
}

impl IntoIterator for LinkMetaKey<'_> {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        (&self).into_iter()
    }
}

impl<T: PrefixType> From<&LinkMetaKey<'_>> for PrefixBytesKey<T> {
    fn from(k: &LinkMetaKey) -> Self {
        PrefixBytesKey::new(k)
    }
}

impl<T: PrefixType> From<LinkMetaKey<'_>> for PrefixBytesKey<T> {
    fn from(k: LinkMetaKey) -> Self {
        (&k).into()
    }
}

impl From<&Header> for ChainItemKey {
    fn from(h: &Header) -> Self {
        ChainItemKey::Full(
            h.author().clone(),
            h.header_seq(),
            HeaderHash::with_data_sync(h),
        )
    }
}

impl From<ChainItemKey> for HeaderHash {
    fn from(c: ChainItemKey) -> Self {
        match c {
            ChainItemKey::Full(_, _, h) => h,
            _ => unreachable!("Tried to get header hash from a partial key: {:?}", c),
        }
    }
}

impl From<&ChainItemKey> for u32 {
    fn from(c: &ChainItemKey) -> Self {
        match c {
            ChainItemKey::AgentSequence(_, s) | ChainItemKey::Full(_, s, _) => *s,
            _ => unreachable!("Tried to get sequence from a partial key: {:?}", c),
        }
    }
}

impl From<&ChainItemKey> for BytesKey {
    fn from(key: &ChainItemKey) -> Self {
        use byteorder::{BigEndian, WriteBytesExt};
        match key {
            ChainItemKey::Agent(a) => a.as_ref().into(),
            ChainItemKey::AgentSequence(a, s) => {
                // Get the agent key
                let mut buf = a.clone().into_inner();
                let mut num = Vec::with_capacity(4);

                // Get the header seq
                num.write_u32::<BigEndian>(*s).unwrap();
                buf.extend(num);
                buf.into()
            }
            ChainItemKey::Full(a, s, h) => {
                // Get the agent key
                let mut buf = a.clone().into_inner();
                let mut num = Vec::with_capacity(4);

                // Get the header seq
                num.write_u32::<BigEndian>(*s).unwrap();
                buf.extend(num);

                // Get the header hash
                buf.extend(h.clone().into_inner());
                buf.into()
            }
        }
    }
}

// TODO: This is way to fragile there must be a better way
// get from the k bytes to the chain item key
impl From<BytesKey> for ChainItemKey {
    fn from(b: BytesKey) -> Self {
        use byteorder::{BigEndian, ByteOrder};
        let bytes = b.0;
        const SEQ_SIZE: usize = std::mem::size_of::<u32>();
        debug_assert_eq!(bytes.len(), HOLO_HASH_SERIALIZED_LEN * 2 + SEQ_SIZE);

        // Tak 36 for the AgentPubKey
        let a = AgentPubKey::from_raw_bytes(bytes[..HOLO_HASH_SERIALIZED_LEN].to_owned());

        // Take another 4 for the u32
        let seq_bytes: Vec<_> =
            bytes[HOLO_HASH_SERIALIZED_LEN..(HOLO_HASH_SERIALIZED_LEN + SEQ_SIZE)].to_owned();
        let s = BigEndian::read_u32(&seq_bytes);

        // Take the rest for the header hash
        let h =
            HeaderHash::from_raw_bytes(bytes[(HOLO_HASH_SERIALIZED_LEN + SEQ_SIZE)..].to_owned());

        ChainItemKey::Full(a, s, h)
    }
}
