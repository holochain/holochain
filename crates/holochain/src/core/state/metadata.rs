#![allow(clippy::ptr_arg)]
//! # Metadata
//! This module is responsible for generating and storing holochain metadata.
//!
//! [Entry]: holochain_types::Entry

use fallible_iterator::FallibleIterator;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{KvBuf, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use holochain_types::header;
use holochain_types::{
    composite_hash::{AnyDhtHash, EntryHash},
    header::{LinkAdd, LinkRemove, ZomeId},
    link::LinkTag,
    Header, HeaderHashed, Timestamp,
};
use std::fmt::Debug;

pub use sys_meta::*;
use tracing::*;

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
#[derive(Debug)]
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
    /// The [ZomeId] of the zome this link belongs to
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
            BaseZome(b, z) => [b.as_ref(), &[*z]].concat(),
            BaseZomeTag(b, z, t) => [b.as_ref(), &[*z], t.as_ref()].concat(),
            Full(b, z, t, l) => [b.as_ref(), &[*z], t.as_ref(), l.as_ref()].concat(),
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
    fn remove_link(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        zome_id: ZomeId,
        tag: LinkTag,
    ) -> DatabaseResult<()>;

    /// Adds a new [EntryCreate] [Header] to an [Entry] in the sys metadata
    async fn add_create(&mut self, create: header::EntryCreate) -> DatabaseResult<()>;

    /// Adds a new [EntryUpdate] [Header] to an [Entry] in the sys metadata
    async fn add_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()>;

    /// Adds a new [EntryDelete] [Header] to an [Entry] in the sys metadata
    async fn add_delete(&mut self, delete: header::EntryDelete) -> DatabaseResult<()>;

    /// Returns all the [HeaderHash]s of [EntryCreate] headers on an [Entry]
    fn get_creates(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the [HeaderHash]s of [EntryUpdates] headers on an [Entry]
    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the [HeaderHash]s of [EntryDeletes] headers on an [Entry]
    fn get_deletes(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Returns the current status of a [Entry]
    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    /// Finds the redirect path and returns the final [Entry]
    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    /// Finds the redirect path and returns the final [Header]
    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
}

/// Values of [Header]s stored by the sys meta db
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {
    /// [EntryCreate] [Header]
    Create(HeaderHash),
    /// [EntryUpdate] [Header]
    Update(HeaderHash),
    /// [EntryDelete] [Header]
    Delete(HeaderHash),
}

/// Subset of headers for the sys meta db
enum EntryHeader {
    Create(Header),
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
            SysMetaVal::Create(h) | SysMetaVal::Update(h) | SysMetaVal::Delete(h) => h,
        }
    }
}

impl EntryHeader {
    async fn into_hash(self) -> Result<HeaderHash, SerializedBytesError> {
        let header = match self {
            EntryHeader::Create(h) => h,
            EntryHeader::Update(h) => h,
            EntryHeader::Delete(h) => h,
        };
        let (_, header_hash): (Header, HeaderHash) = HeaderHashed::with_data(header).await?.into();
        Ok(header_hash)
    }
}

impl From<header::EntryCreate> for EntryHeader {
    fn from(h: header::EntryCreate) -> Self {
        EntryHeader::Create(Header::EntryCreate(h))
    }
}

impl From<header::EntryUpdate> for EntryHeader {
    fn from(h: header::EntryUpdate) -> Self {
        EntryHeader::Update(Header::EntryUpdate(h))
    }
}

impl From<header::EntryDelete> for EntryHeader {
    fn from(h: header::EntryDelete) -> Self {
        EntryHeader::Delete(Header::EntryDelete(h))
    }
}

/// Updates and answers queries for the links and system meta databases
pub struct MetadataBuf<'env> {
    system_meta: KvvBuf<'env, SysMetaKey, SysMetaVal, Reader<'env>>,
    links_meta: KvBuf<'env, LinkMetaKeyBytes, LinkMetaVal, Reader<'env>>,
}

impl<'env> MetadataBuf<'env> {
    pub(crate) fn new(
        reader: &'env Reader<'env>,
        system_meta: MultiStore,
        links_meta: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBuf::new(reader, system_meta)?,
            links_meta: KvBuf::new(reader, links_meta)?,
        })
    }
    /// Create a [MetadataBuf] with the primary databases
    pub fn primary(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*PRIMARY_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*PRIMARY_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

    /// Create a [MetadataBuf] with the cache databases
    pub fn cache(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

    async fn add_entry_header<K, H>(&mut self, header: H, key: K) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::Create(_) => SysMetaVal::Create(h.into_hash().await?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash().await?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash().await?),
        };
        self.system_meta.insert(key.into(), sys_val);
        Ok(())
    }
}

#[async_trait::async_trait]
impl<'env> MetadataBufT for MetadataBuf<'env> {
    fn get_links<'a>(&self, key: &'a LinkMetaKey) -> DatabaseResult<Vec<LinkMetaVal>> {
        self.links_meta
            .iter_all_key_matches(key.to_key())?
            .map(|(_, v)| Ok(v))
            .collect()
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        let (_, link_add_hash): (Header, HeaderHash) =
            HeaderHashed::with_data(Header::LinkAdd(link_add.clone()))
                .await?
                .into();
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

    fn remove_link(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        zome_id: ZomeId,
        tag: LinkTag,
    ) -> DatabaseResult<()> {
        let key = LinkMetaKey::Full(base, zome_id, &tag, &link_remove.link_add_address);
        debug!(removing_key = ?key);
        // TODO: It should be impossible to ever remove a LinkMetaVal that wasn't already added
        // because of the validation dependency on LinkAdd from LinkRemove
        // but do we want some kind of warning or panic here incase we messed up?
        self.links_meta.delete(key.to_key())
    }

    // TODO: Add register_header

    // TODO: This no longer makes sense wih register_header
    #[allow(clippy::needless_lifetimes)]
    async fn add_create(&mut self, create: header::EntryCreate) -> DatabaseResult<()> {
        let entry_hash = create.entry_hash.to_owned();
        self.add_entry_header(create, entry_hash).await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()> {
        let basis: AnyDhtHash = match (&update.update_basis, entry) {
            (header::UpdateBasis::Header, None) => update.replaces_address.clone().into(),
            (header::UpdateBasis::Header, Some(_)) => {
                panic!("Can't update to entry when EntryUpdate points to header")
            }
            (header::UpdateBasis::Entry, None) => {
                panic!("Can't update to entry with no entry hash")
            }
            (header::UpdateBasis::Entry, Some(entry_hash)) => entry_hash.into(),
        };
        self.add_entry_header(update, basis).await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_delete(&mut self, delete: header::EntryDelete) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        self.add_entry_header(delete, remove).await
    }

    // TODO: Add get_headers
    // Remove as this makes no sense with get_headers
    fn get_creates(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&entry_hash.into())?)
                .map(|h| Ok(h.into())),
        ))
    }

    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&hash)?).map(|h| Ok(h.into())),
        ))
    }

    fn get_deletes(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(&header_hash.into())?)
                .map(|h| Ok(h.into())),
        ))
    }

    // TODO: For now this isn't actually checking the meta data.
    // Once the meta data is finished this should be hooked up
    fn get_dht_status(&self, _entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        // if fallible_iterator::convert(self.system_meta.get(&entry_hash.clone().into())?)
        //     .filter(|sys_val| {
        //         if let SysMetaVal::Create(_) = sys_val {
        //             Ok(true)
        //         } else {
        //             Ok(false)
        //         }
        //     })
        //     .count()?
        //     > 0
        // {
        //     Ok(EntryDhtStatus::Live)
        // } else {
        //     Ok(EntryDhtStatus::Dead)
        // }
        Ok(EntryDhtStatus::Live)
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!()
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!()
    }
}

impl<'env> BufferedStore<'env> for MetadataBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        Ok(())
    }
}
