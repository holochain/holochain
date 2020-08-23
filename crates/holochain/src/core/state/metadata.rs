#![allow(clippy::ptr_arg)]
//! # Metadata
//! This module is responsible for generating and storing holochain metadata.
//!
//! [Entry]: holochain_types::Entry

use fallible_iterator::FallibleIterator;
use holo_hash::HasHash;
use holo_hash::{AgentPubKey, AnyDhtHash, EntryHash, HeaderHash};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{KvBufUsed, KvvBufUsed},
    db::{
        CACHE_LINKS_META, CACHE_STATUS_META, CACHE_SYSTEM_META, META_VAULT_LINKS,
        META_VAULT_STATUS, META_VAULT_SYS,
    },
    error::{DatabaseError, DatabaseResult},
    fresh_reader,
    prelude::*,
};
use holochain_types::metadata::{EntryDhtStatus, TimedHeaderHash};
use holochain_types::{HeaderHashed, Timestamp};
use holochain_zome_types::header::{self, LinkAdd, LinkRemove, ZomeId};
use holochain_zome_types::{link::LinkTag, Header};
use std::fmt::Debug;

pub use sys_meta::*;
use tracing::*;

use holochain_types::{header::NewEntryHeader, link::WireLinkMetaKey};

#[cfg(test)]
pub use mock::MockMetadataBuf;
#[cfg(test)]
use mockall::mock;

#[cfg(test)]
pub mod links_test;
mod sys_meta;

#[allow(missing_docs)]
#[cfg(test)]
mod mock;

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

/// The actual type the [LinkMetaKey] turns into
#[derive(Ord, PartialOrd, Eq, PartialEq, derive_more::From)]
struct LinkMetaKeyBytes(Vec<u8>);

impl AsRef<[u8]> for LinkMetaKeyBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl BufKey for LinkMetaKeyBytes {
    fn from_key_bytes_or_friendly_panic(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }
}

impl<'a> LinkMetaKey<'a> {
    fn to_key(&self) -> LinkMetaKeyBytes {
        use LinkMetaKey::*;
        match self {
            Base(b) => b.as_ref().to_vec(),
            BaseZome(b, z) => [b.as_ref(), &[u8::from(*z)]].concat(),
            BaseZomeTag(b, z, t) => [b.as_ref(), &[u8::from(*z)], t.as_ref()].concat(),
            Full(b, z, t, l) => [b.as_ref(), &[u8::from(*z)], t.as_ref(), l.as_ref()].concat(),
        }
        .into()
    }

    /// Return the base of this key
    pub fn base(&self) -> &EntryHash {
        use LinkMetaKey::*;
        match self {
            Base(b) | BaseZome(b, _) | BaseZomeTag(b, _, _) | Full(b, _, _, _) => b,
        }
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

/// Trait for the [MetadataBuf]
/// Needed for mocking
#[async_trait::async_trait]
pub trait MetadataBufT {
    // Links
    /// Get all the links on this base that match the tag
    /// that do not have removes on them
    fn get_live_links<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>;

    /// Get all the links on this base that match the tag regardless of removes
    fn get_links_all<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>;

    /// Add a link
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Register a HeaderHash directly on an entry hash.
    /// Also updates the entry dht status.
    /// Useful when you only have hashes and not full types
    async fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()>;

    /// Register a HeaderHash directly on a header hash.
    /// Useful when you only have hashes and not full types
    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);

    /// Remove a link
    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()>;

    /// Registers a [Header::NewEntryHeader] on the referenced [Entry]
    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Registers a published [Header] on the authoring agent's public key
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()>;

    /// Registers a [Header::EntryUpdate] on the referenced [Header] or [Entry]
    async fn register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()>;

    /// Registers a [Header::ElementDelete] on the Header of an Entry
    async fn register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()>;

    /// Returns all the [HeaderHash]es of headers that created this [Entry]
    fn get_headers<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all headers registered on an agent's public key
    fn get_activity<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [EntryUpdate] headers registered on an [Entry]
    fn get_updates<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on a Header
    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on an Entry's header
    fn get_deletes_on_entry<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns the current [EntryDhtStatus] of an [Entry]
    fn get_dht_status<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus>;

    /// Finds the redirect path and returns the final [Entry]
    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    /// Finds the redirect path and returns the final [Header]
    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;

    /// Returns all the link remove headers attached to a link add header
    fn get_link_removes_on_link_add<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;
}

/// Values of [Header]s stored by the sys meta db
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Serialize, Deserialize)]
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

/// Subset of headers for the sys meta db
enum EntryHeader {
    Activity(Header),
    NewEntry(Header),
    Update(Header),
    Delete(Header),
}

type SysMetaKey = AnyDhtHash;

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

impl EntryHeader {
    async fn into_hash(self) -> Result<TimedHeaderHash, SerializedBytesError> {
        let header = match self {
            EntryHeader::NewEntry(h)
            | EntryHeader::Update(h)
            | EntryHeader::Delete(h)
            | EntryHeader::Activity(h) => h,
        };
        let (header, header_hash): (Header, HeaderHash) = HeaderHashed::from_content(header).into();
        Ok(TimedHeaderHash {
            timestamp: header.timestamp().into(),
            header_hash,
        })
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

/// Updates and answers queries for the links and system meta databases
pub struct MetadataBuf {
    system_meta: KvvBufUsed<SysMetaKey, SysMetaVal>,
    links_meta: KvBufUsed<LinkMetaKeyBytes, LinkMetaVal>,
    status_meta: KvBufUsed<EntryHash, EntryDhtStatus>,
    env: EnvironmentRead,
}

impl MetadataBuf {
    pub(crate) fn new(
        env: EnvironmentRead,
        system_meta: MultiStore,
        links_meta: SingleStore,
        status_meta: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBufUsed::new(system_meta),
            links_meta: KvBufUsed::new(links_meta),
            status_meta: KvBufUsed::new(status_meta),
            env,
        })
    }
    /// Create a [MetadataBuf] with the vault databases
    pub fn vault(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*META_VAULT_SYS)?;
        let links_meta = dbs.get_db(&*META_VAULT_LINKS)?;
        let status_meta = dbs.get_db(&*META_VAULT_STATUS)?;
        Self::new(env, system_meta, links_meta, status_meta)
    }

    /// Create a [MetadataBuf] with the cache databases
    pub fn cache(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        let status_meta = dbs.get_db(&*CACHE_STATUS_META)?;
        Self::new(env, system_meta, links_meta, status_meta)
    }

    async fn register_header_on_basis<K, H>(&mut self, key: K, header: H) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::NewEntry(_) => SysMetaVal::NewEntry(h.into_hash().await?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash().await?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash().await?),
            h @ EntryHeader::Activity(_) => SysMetaVal::Activity(h.into_hash().await?),
        };
        self.system_meta.insert(key.into(), sys_val);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn update_entry_dht_status(&mut self, basis: EntryHash) -> DatabaseResult<()> {
        let status = fresh_reader!(self.env, |r| self.get_headers(&r, basis.clone())?.find_map(
            |header| {
                if let None = self.get_deletes_on_header(&r, header.header_hash)?.next()? {
                    debug!("found live header");
                    Ok(Some(EntryDhtStatus::Live))
                } else {
                    debug!("found dead header");
                    Ok(None)
                }
            }
        ))?
        // No evidence of life found so entry is marked dead
        .unwrap_or(EntryDhtStatus::Dead);
        self.status_meta.put(basis, status)
    }

    #[cfg(test)]
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.links_meta.clear_all(writer)?;
        self.system_meta.clear_all(writer)
    }
}

#[async_trait::async_trait]
impl MetadataBufT for MetadataBuf {
    fn get_live_links<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        Ok(Box::new(
            self.links_meta
                .iter_all_key_matches(r, key.to_key())?
                .filter_map(move |(_, link)| {
                    // Check if link has been removed
                    match self
                        .get_link_removes_on_link_add(r, link.link_add_hash.clone())?
                        .next()?
                    {
                        Some(_) => Ok(None),
                        None => Ok(Some(link)),
                    }
                }),
        ))
    }

    fn get_links_all<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        Ok(Box::new(
            self.links_meta
                .iter_all_key_matches(r, key.to_key())?
                .map(|(_, v)| Ok(v)),
        ))
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        // Register the add link onto the base
        let link_add_hash =
            HeaderHashed::from_content(Header::LinkAdd(link_add.clone())).into_hash();

        // Put the link add to the links table
        let key = LinkMetaKey::from((&link_add, &link_add_hash));

        self.links_meta.put(
            key.to_key(),
            LinkMetaVal {
                link_add_hash,
                target: link_add.target_address,
                timestamp: link_add.timestamp.into(),
                zome_id: link_add.zome_id,
                tag: link_add.tag,
            },
        )
    }

    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()> {
        let link_add_address = link_remove.link_add_address.clone();
        // Register the link remove address to the link add address
        let link_remove = HeaderHashed::from_content(Header::LinkRemove(link_remove));
        let sys_val = SysMetaVal::LinkRemove(link_remove.into());
        self.system_meta.insert(link_add_address.into(), sys_val);
        Ok(())
    }

    async fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.system_meta.insert(entry_hash.clone().into(), value);
        self.update_entry_dht_status(entry_hash).await
    }

    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.system_meta.insert(header_hash.into(), value);
    }

    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.register_header_on_basis(basis.clone(), new_entry_header)
            .await?;
        self.update_entry_dht_status(basis).await?;
        Ok(())
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()> {
        self.register_header_on_basis(
            AnyDhtHash::from(update.original_entry_address.clone()),
            update,
        )
        .await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        let entry_hash = delete.removes_entry_address.clone();
        self.register_header_on_basis(remove, delete.clone())
            .await?;
        self.register_header_on_basis(entry_hash.clone(), delete)
            .await?;
        self.update_entry_dht_status(entry_hash).await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()> {
        let author = header.author().clone();
        self.register_header_on_basis(author, EntryHeader::Activity(header))
            .await
    }

    fn get_headers<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &entry_hash.into())?).filter_map(
                |h| {
                    Ok(match h {
                        SysMetaVal::NewEntry(h) => Some(h),
                        _ => None,
                    })
                },
            ),
        ))
    }

    fn get_updates<'r, R: Readable>(
        &'r self,
        r: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &hash)?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Update(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        r: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &new_entry_header.into())?)
                .filter_map(|h| {
                    Ok(match h {
                        SysMetaVal::Delete(h) => Some(h),
                        _ => None,
                    })
                }),
        ))
    }

    fn get_deletes_on_entry<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &entry_hash.into())?).filter_map(
                |h| {
                    Ok(match h {
                        SysMetaVal::Delete(h) => Some(h),
                        _ => None,
                    })
                },
            ),
        ))
    }

    fn get_activity<'r, R: Readable>(
        &'r self,
        r: &'r R,
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &agent_pubkey.into())?).filter_map(
                |h| {
                    Ok(match h {
                        SysMetaVal::Activity(h) => Some(h),
                        _ => None,
                    })
                },
            ),
        ))
    }

    // TODO: For now this is only checking for deletes
    // Once the validation is finished this should check for that as well
    fn get_dht_status<'r, R: Readable>(
        &self,
        r: &'r R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus> {
        Ok(self
            .status_meta
            .get(r, entry_hash)?
            .unwrap_or(EntryDhtStatus::Dead))
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!()
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!()
    }

    fn get_link_removes_on_link_add<'r, R: Readable>(
        &'r self,
        r: &'r R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &link_add.into())?).filter_map(
                |h| {
                    Ok(match h {
                        SysMetaVal::LinkRemove(h) => Some(h),
                        _ => None,
                    })
                },
            ),
        ))
    }
}

impl BufferedStore for MetadataBuf {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        self.status_meta.flush_to_txn(writer)?;
        Ok(())
    }
}
