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
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct LinkMetaVal {
    /// Hash of the [LinkAdd] [Header] that created this link
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
    /// This will match only the link created with a certain [LinkAdd] hash
    Full(&'a EntryHash, ZomeId, &'a LinkTag, &'a HeaderHash),
}

pub(super) type SysMetaKey = AnyDhtHash;

/// Values of [Header]s stored by the sys meta db
#[derive(Debug, Hash, PartialEq, Eq, Ord, PartialOrd, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {
    /// A header that results in a new entry
    /// Either a [EntryCreate] or [EntryUpdate]
    NewEntry(TimedHeaderHash),
    /// An [EntryUpdate] [Header]
    Update(TimedHeaderHash),
    /// An [Header::ElementDelete]
    Delete(TimedHeaderHash),
    /// Activity on an agent's public key
    Activity(TimedHeaderHash),
    /// Link remove on link add
    LinkRemove(TimedHeaderHash),
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

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
/// Values for the misc kv
/// Matches the key
pub(super) enum MiscMetaValue {
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
    fn from_key_bytes_fallible(bytes: &[u8]) -> Self {
        bytes.into()
    }
}

impl EntryHeader {
    pub(super) async fn into_hash(self) -> Result<TimedHeaderHash, SerializedBytesError> {
        let header = match self {
            EntryHeader::NewEntry(h)
            | EntryHeader::Update(h)
            | EntryHeader::Delete(h)
            | EntryHeader::Activity(h) => h,
        };
        let (header, header_hash): (Header, HeaderHash) =
            HeaderHashed::from_content(header).await.into();
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
    fn from(k: &LinkMetaKey<'_>) -> Self {
        use LinkMetaKey::*;
        match k {
            Base(b) => b.as_ref().to_vec(),
            BaseZome(b, z) => [b.as_ref(), &[u8::from(*z)]].concat(),
            BaseZomeTag(b, z, t) => [b.as_ref(), &[u8::from(*z)], t.as_ref()].concat(),
            Full(b, z, t, l) => [b.as_ref(), &[u8::from(*z)], t.as_ref(), l.as_ref()].concat(),
        }
        .into()
    }
}

impl From<LinkMetaKey<'_>> for BytesKey {
    fn from(k: LinkMetaKey<'_>) -> Self {
        (&k).into()
    }
}

impl From<SysMetaVal> for HeaderHash {
    fn from(v: SysMetaVal) -> Self {
        match v {
            SysMetaVal::NewEntry(h)
            | SysMetaVal::Update(h)
            | SysMetaVal::Delete(h)
            | SysMetaVal::LinkRemove(h)
            | SysMetaVal::Activity(h) => h.header_hash,
        }
    }
}

impl From<NewEntryHeader> for EntryHeader {
    fn from(h: NewEntryHeader) -> Self {
        EntryHeader::NewEntry(h.into())
    }
}

impl From<header::EntryUpdate> for EntryHeader {
    fn from(h: header::EntryUpdate) -> Self {
        EntryHeader::Update(Header::EntryUpdate(h))
    }
}

impl From<header::ElementDelete> for EntryHeader {
    fn from(h: header::ElementDelete) -> Self {
        EntryHeader::Delete(Header::ElementDelete(h))
    }
}

impl<'a> From<(&'a LinkAdd, &'a HeaderHash)> for LinkMetaKey<'a> {
    fn from((link_add, hash): (&'a LinkAdd, &'a HeaderHash)) -> Self {
        Self::Full(
            &link_add.base_address,
            link_add.zome_id,
            &link_add.tag,
            hash,
        )
    }
}

impl<'a> From<&'a WireLinkMetaKey> for LinkMetaKey<'a> {
    fn from(w: &'a WireLinkMetaKey) -> Self {
        match w {
            WireLinkMetaKey::Base(b) => Self::Base(b),
            WireLinkMetaKey::BaseZome(b, z) => Self::BaseZome(b, *z),
            WireLinkMetaKey::BaseZomeTag(b, z, t) => Self::BaseZomeTag(b, *z, t),
            WireLinkMetaKey::Full(b, z, t, l) => Self::Full(b, *z, t, l),
        }
    }
}

impl From<&LinkMetaKey<'_>> for WireLinkMetaKey {
    fn from(k: &LinkMetaKey) -> Self {
        match k.clone() {
            LinkMetaKey::Base(b) => Self::Base(b.clone()),
            LinkMetaKey::BaseZome(b, z) => Self::BaseZome(b.clone(), z),
            LinkMetaKey::BaseZomeTag(b, z, t) => Self::BaseZomeTag(b.clone(), z, t.clone()),
            LinkMetaKey::Full(b, z, t, l) => Self::Full(b.clone(), z, t.clone(), l.clone()),
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
