#![allow(clippy::ptr_arg)]
//! # Metadata
//! This module is responsible for generating and storing holochain metadata.
//!
//! [Entry]: holochain_types::Entry

use fallible_iterator::FallibleIterator;
use holo_hash::{AgentPubKey, Hashed, HeaderHash};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{KvBuf, KvvBuf},
    db::{
        CACHE_LINKS_META, CACHE_STATUS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META,
        PRIMARY_STATUS_META, PRIMARY_SYSTEM_META,
    },
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use holochain_types::header;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    header::{LinkAdd, LinkRemove, ZomeId},
    Header, HeaderHashed, Timestamp,
};
use holochain_zome_types::link::LinkTag;
use std::fmt::Debug;

pub use sys_meta::*;
use tracing::*;

use header::NewEntryHeader;
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

/// The status of an [Entry] in the Dht
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryDhtStatus {
    /// This [Entry] has active headers
    Live,
    /// This [Entry] has no headers that have not been deleted
    Dead,
    /// This [Entry] is awaiting validation
    Pending,
    /// This [Entry] has failed validation and will not be served by the DHT
    Rejected,
    /// This [Entry] has taken too long / too many resources to validate, so we gave up
    Abandoned,
    /// **not implemented** There has been a conflict when validating this [Entry]
    Conflict,
    /// **not implemented** The author has withdrawn their publication of this element.
    Withdrawn,
    /// **not implemented** We have agreed to drop this [Entry] content from the system. Header can stay with no entry
    Purged,
}

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
type LinkMetaKeyBytes = Vec<u8>;

impl<'a> LinkMetaKey<'a> {
    fn to_key(&self) -> LinkMetaKeyBytes {
        use LinkMetaKey::*;
        match self {
            Base(b) => b.as_ref().to_vec(),
            BaseZome(b, z) => [b.as_ref(), &[u8::from(*z)]].concat(),
            BaseZomeTag(b, z, t) => [b.as_ref(), &[u8::from(*z)], t.as_ref()].concat(),
            Full(b, z, t, l) => [b.as_ref(), &[u8::from(*z)], t.as_ref(), l.as_ref()].concat(),
        }
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

impl LinkMetaVal {
    /// Turn into a zome friendly type
    pub fn into_link(self) -> holochain_zome_types::link::Link {
        let timestamp: chrono::DateTime<chrono::Utc> = self.timestamp.into();
        holochain_zome_types::link::Link {
            target: self.target.into(),
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
    fn get_links<'a>(&self, key: &'a LinkMetaKey) -> DatabaseResult<Vec<LinkMetaVal>>;

    /// Add a link
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Remove a link
    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()>;

    /// Registers a [Header::NewEntryHeader] on the referenced [Entry]
    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Registers a published [Header] on the authoring agent's public key
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()>;

    /// Registers a [Header::EntryUpdate] on the referenced [Header] or [Entry]
    async fn register_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()>;

    /// Registers a [Header::ElementDelete] on the Header of an Entry
    async fn register_delete(
        &mut self,
        delete: header::ElementDelete,
        entry_hash: EntryHash,
    ) -> DatabaseResult<()>;

    /// Returns all the [HeaderHash]es of headers that created this [Entry]
    fn get_headers(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all headers registered on an agent's public key
    fn get_activity(
        &self,
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [EntryUpdate] headers registered on an [Entry]
    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on a Header
    fn get_deletes_on_header(
        &self,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on an Entry's header
    fn get_deletes_on_entry(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns the current [EntryDhtStatus] of an [Entry]
    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    /// Finds the redirect path and returns the final [Entry]
    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    /// Finds the redirect path and returns the final [Header]
    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;

    /// Get link removes on link adds
    fn get_link_remove_on_link_add(
        &self,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;
}

/// Values of [Header]s stored by the sys meta db
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {
    /// A header that results in a new entry
    /// Either a [EntryCreate] or [EntryUpdate]
    NewEntry(HeaderHash),
    /// An [EntryUpdate] [Header]
    Update(HeaderHash),
    /// An [Header::ElementDelete]
    Delete(HeaderHash),
    /// Activity on an agent's public key
    Activity(HeaderHash),
    /// Link remove on link add
    LinkRemove(HeaderHash),
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
            | SysMetaVal::Activity(h) => h,
        }
    }
}

impl EntryHeader {
    async fn into_hash(self) -> Result<HeaderHash, SerializedBytesError> {
        let header = match self {
            EntryHeader::NewEntry(h)
            | EntryHeader::Update(h)
            | EntryHeader::Delete(h)
            | EntryHeader::Activity(h) => h,
        };
        Ok(HeaderHashed::with_data(header).await?.into_hash())
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
pub struct MetadataBuf<'env> {
    system_meta: KvvBuf<'env, SysMetaKey, SysMetaVal, Reader<'env>>,
    links_meta: KvBuf<'env, LinkMetaKeyBytes, LinkMetaVal, Reader<'env>>,
    status_meta: KvBuf<'env, EntryHash, EntryDhtStatus, Reader<'env>>,
}

impl<'env> MetadataBuf<'env> {
    pub(crate) fn new(
        reader: &'env Reader<'env>,
        system_meta: MultiStore,
        links_meta: SingleStore,
        status_meta: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBuf::new(reader, system_meta)?,
            links_meta: KvBuf::new(reader, links_meta)?,
            status_meta: KvBuf::new(reader, status_meta)?,
        })
    }
    /// Create a [MetadataBuf] with the primary databases
    pub fn primary(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*PRIMARY_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*PRIMARY_LINKS_META)?;
        let status_meta = dbs.get_db(&*PRIMARY_STATUS_META)?;
        Self::new(reader, system_meta, links_meta, status_meta)
    }

    /// Create a [MetadataBuf] with the cache databases
    pub fn cache(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        let status_meta = dbs.get_db(&*CACHE_STATUS_META)?;
        Self::new(reader, system_meta, links_meta, status_meta)
    }

    async fn register_header_to_basis<K, H>(&mut self, header: H, key: K) -> DatabaseResult<()>
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
    fn update_entry_dht_status(&mut self, basis: EntryHash) -> DatabaseResult<()> {
        let status = self
            .get_headers(basis.clone())?
            .find_map(|header| {
                if let None = self.get_deletes_on_header(header)?.next()? {
                    debug!("found live header");
                    Ok(Some(EntryDhtStatus::Live))
                } else {
                    debug!("found dead header");
                    Ok(None)
                }
            })?
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
impl<'env> MetadataBufT for MetadataBuf<'env> {
    fn get_links<'a>(&self, key: &'a LinkMetaKey) -> DatabaseResult<Vec<LinkMetaVal>> {
        self.links_meta
            .iter_all_key_matches(key.to_key())?
            .filter_map(|(_, link)| {
                // Check if link has been removed
                match self
                    .get_link_remove_on_link_add(link.link_add_hash.clone())?
                    .next()?
                {
                    Some(_) => Ok(None),
                    None => Ok(Some(link)),
                }
            })
            .collect()
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        // Register the add link onto the base
        let link_add_hash = HeaderHashed::with_data(Header::LinkAdd(link_add.clone()))
            .await?
            .into_hash();

        // Put the link add to the links table
        let key = LinkMetaKey::from((&link_add, &link_add_hash));

        self.links_meta.put(
            key.to_key(),
            LinkMetaVal {
                link_add_hash,
                target: link_add.target_address,
                timestamp: link_add.timestamp,
                zome_id: link_add.zome_id,
                tag: link_add.tag,
            },
        )
    }

    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()> {
        // Register the link remove address to the link add address
        let link_remove_address = HeaderHashed::with_data(Header::LinkRemove(link_remove.clone()))
            .await?
            .into_hash();
        let sys_val = SysMetaVal::LinkRemove(link_remove_address);
        self.system_meta
            .insert(link_remove.link_add_address.clone().into(), sys_val);
        Ok(())
    }

    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.register_header_to_basis(new_entry_header, basis.clone())
            .await?;
        self.update_entry_dht_status(basis)?;
        Ok(())
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()> {
        let basis: AnyDhtHash = match (&update.intended_for, entry) {
            (header::IntendedFor::Header, None) => update.replaces_address.clone().into(),
            (header::IntendedFor::Header, Some(_)) => {
                panic!("Can't update to entry when EntryUpdate points to header")
            }
            (header::IntendedFor::Entry, None) => {
                panic!("Can't update to entry with no entry hash")
            }
            (header::IntendedFor::Entry, Some(entry_hash)) => {
                // TODO: Can an update intended for a header also change an
                // entries dht status?
                self.update_entry_dht_status(entry_hash.clone())?;
                entry_hash.into()
            }
        };
        self.register_header_to_basis(update, basis).await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_delete(
        &mut self,
        delete: header::ElementDelete,
        entry_hash: EntryHash,
    ) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        self.register_header_to_basis(delete.clone(), remove)
            .await?;
        self.register_header_to_basis(delete, entry_hash.clone())
            .await?;
        self.update_entry_dht_status(entry_hash)
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()> {
        let author = header.author().clone();
        self.register_header_to_basis(EntryHeader::Activity(header), author)
            .await
    }

    fn get_headers(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&entry_hash.into())?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::NewEntry(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&hash)?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Update(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_deletes_on_header(
        &self,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&new_entry_header.into())?).filter_map(
                |h| {
                    Ok(match h {
                        SysMetaVal::Delete(h) => Some(h),
                        _ => None,
                    })
                },
            ),
        ))
    }

    fn get_deletes_on_entry(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&entry_hash.into())?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Delete(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_activity(
        &self,
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&agent_pubkey.into())?).filter_map(
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
    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        Ok(self
            .status_meta
            .get(entry_hash)?
            .unwrap_or(EntryDhtStatus::Live))
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!()
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!()
    }

    /// Get link removes on link adds
    fn get_link_remove_on_link_add(
        &self,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&link_add.into())?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::LinkRemove(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }
}

impl<'env> BufferedStore<'env> for MetadataBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        self.status_meta.flush_to_txn(writer)?;
        Ok(())
    }
}
