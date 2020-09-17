use super::*;
/// Some keys do not store an array of bytes
/// so can not impl AsRef<[u8]>.
/// This is the key type for those keys to impl into
#[derive(
    Ord, PartialOrd, Eq, PartialEq, derive_more::Into, derive_more::From, derive_more::AsRef,
)]
#[as_ref(forward)]
pub struct BytesKey(pub Vec<u8>);

/// The value stored in the links meta db
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
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

/// The typed representation of a SysMeta key.
/// However, this must be converted to a PrefixBytesKey before inserting into a DB.
pub type SysMetaKey = AnyDhtHash;

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
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, SerializedBytes)]
/// Key for the misc metadata kv
/// This holds miscellaneous data relevant
/// to the metadata store
pub(super) enum MiscMetaKey {
    /// Collapsed status of an entry
    EntryStatus(EntryHash),
    /// We have integrated a StoreElement for this key
    StoreElement(HeaderHash),
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
/// Values for the misc kv
/// Matches the key
pub enum MiscMetaValue {
    /// Collapsed status of an entry
    EntryStatus(EntryDhtStatus),
    /// We have integrated a StoreElement for this key
    StoreElement(()),
}

/// Subset of headers for the sys meta db
pub(super) enum EntryHeader {
    Activity(Header),
    NewEntry(Header),
    Update(Header),
    Delete(Header),
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
            EntryHeader::NewEntry(h)
            | EntryHeader::Update(h)
            | EntryHeader::Delete(h)
            | EntryHeader::Activity(h) => h,
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

impl MiscMetaValue {
    pub(super) fn entry_status(self) -> EntryDhtStatus {
        match self {
            MiscMetaValue::EntryStatus(e) => e,
            _ => unreachable!("Tried to go from {:?} to {:?}", self, "entry_status"),
        }
    }

    pub(super) fn new_store_element() -> Self {
        Self::StoreElement(())
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

impl From<MiscMetaKey> for BytesKey {
    fn from(k: MiscMetaKey) -> Self {
        BytesKey::from(&k)
    }
}

impl From<&MiscMetaKey> for BytesKey {
    fn from(k: &MiscMetaKey) -> Self {
        let r: Vec<u8> =
            UnsafeBytes::from(SerializedBytes::try_from(k).expect("Type can't fail to serialize"))
                .into();
        r.into()
    }
}

impl From<BytesKey> for MiscMetaKey {
    fn from(k: BytesKey) -> Self {
        SerializedBytes::from(UnsafeBytes::from(<Vec<u8>>::from(k)))
            .try_into()
            .expect("Database MiscMetaKey failed to serialize")
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

impl IntoIterator for &MiscMetaKey {
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

impl IntoIterator for MiscMetaKey {
    type Item = u8;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        (&self).into_iter()
    }
}

// impl<T: PrefixType> From<MiscMetaKey> for PrefixBytesKey<T> {
//     fn from(k: MiscMetaKey) -> Self {
//         PrefixBytesKey::new(k)
//     }
// }

// impl<T: PrefixType> From<&LinkMetaKey<'_>> for PrefixBytesKey<T> {
//     fn from(k: &LinkMetaKey) -> Self {
//         PrefixBytesKey::new(k)
//     }
// }

// impl<T: PrefixType> From<LinkMetaKey<'_>> for PrefixBytesKey<T> {
//     fn from(k: LinkMetaKey) -> Self {
//         (&k).into()
//     }
// }
