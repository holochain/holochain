#![allow(clippy::ptr_arg)]
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
    link::Tag,
    Header, HeaderHashed, Timestamp,
};
use mockall::mock;
use std::fmt::Debug;

pub use sys_meta::*;
use tracing::*;

#[cfg(test)]
pub mod links_test;
mod sys_meta;

#[derive(Debug)]
pub enum EntryDhtStatus {
    Live,
    Dead,
    Pending,
    Rejected,
    Abandoned,
    Conflict,
    Withdrawn,
    Purged,
}

// TODO: Maybe this should be moved to link.rs in types?
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct LinkMetaVal {
    pub link_add_hash: HeaderHash,
    pub target: EntryHash,
    pub timestamp: Timestamp,
    pub zome_id: ZomeId,
    pub tag: Tag,
}

/// Key for the LinkMeta database.
///
/// Constructed so that links can be queried by a prefix match
/// on the key.
/// Must provide `tag` and `link_add_hash` for inserts,
/// but both are optional for gets.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum LinkMetaKey<'a> {
    Base(&'a EntryHash),
    BaseZome(&'a EntryHash, ZomeId),
    BaseZomeTag(&'a EntryHash, ZomeId, &'a Tag),
    Full(&'a EntryHash, ZomeId, &'a Tag, &'a HeaderHash),
}

type LinkKey = Vec<u8>;

impl<'a> LinkMetaKey<'a> {
    fn to_key(&self) -> LinkKey {
        use LinkMetaKey::*;
        match self {
            Base(b) => b.as_ref().to_vec(),
            BaseZome(b, z) => [b.as_ref(), &[*z]].concat(),
            BaseZomeTag(b, z, t) => [b.as_ref(), &[*z], t.as_ref()].concat(),
            Full(b, z, t, l) => [b.as_ref(), &[*z], t.as_ref(), l.as_ref()].concat(),
        }
    }

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
        tag: Tag,
    ) -> DatabaseResult<()>;

    async fn add_create(&mut self, create: header::EntryCreate) -> DatabaseResult<()>;

    async fn add_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()>;

    async fn add_delete(&mut self, delete: header::EntryDelete) -> DatabaseResult<()>;

    fn get_creates(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>;

    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>;

    fn get_deletes(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>;

    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {
    Create(HeaderHash),
    Update(HeaderHash),
    Delete(HeaderHash),
}

enum EntryHeader {
    Create(Header),
    Update(Header),
    Delete(Header),
}

type SysMetaKey = AnyDhtHash;

impl LinkMetaVal {
    pub fn new(
        link_add_hash: HeaderHash,
        target: EntryHash,
        timestamp: Timestamp,
        zome_id: ZomeId,
        tag: Tag,
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

pub struct MetadataBuf<'env> {
    system_meta: KvvBuf<'env, SysMetaKey, SysMetaVal, Reader<'env>>,
    links_meta: KvBuf<'env, LinkKey, LinkMetaVal, Reader<'env>>,
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
    pub fn primary(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*PRIMARY_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*PRIMARY_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

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
        tag: Tag,
    ) -> DatabaseResult<()> {
        let key = LinkMetaKey::Full(base, zome_id, &tag, &link_remove.link_add_address);
        debug!(removing_key = ?key);
        // TODO: It should be impossible to ever remove a LinkMetaVal that wasn't already added
        // because of the validation dependency on LinkAdd from LinkRemove
        // but do we want some kind of warning or panic here incase we mssed up?
        self.links_meta.delete(key.to_key())
    }

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
        let replace: AnyDhtHash = match (&update.updates_to, entry) {
            (header::UpdatesTo::Header, None) => update.replaces_address.clone().into(),
            (header::UpdatesTo::Header, Some(_)) => {
                panic!("Can't update to entry when EntryUpdate points to header")
            }
            (header::UpdatesTo::Entry, None) => panic!("Can't update to entry with no entry hash"),
            (header::UpdatesTo::Entry, Some(entry_hash)) => entry_hash.into(),
        };
        // let replace = update.replaces_address.to_owned();
        self.add_entry_header(update, replace).await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn add_delete(&mut self, delete: header::EntryDelete) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        self.add_entry_header(delete, remove).await
    }

    fn get_creates(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(fallible_iterator::convert(
            self.system_meta.get(&entry_hash.into())?,
        )))
    }

    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(fallible_iterator::convert(
            self.system_meta.get(&hash)?,
        )))
    }

    fn get_deletes(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(fallible_iterator::convert(
            self.system_meta.get(&header_hash.into())?,
        )))
    }

    // TODO: For now this isn't actually checking the meta data.
    // Once the meta data is finished this should be hooked up
    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        if fallible_iterator::convert(self.system_meta.get(&entry_hash.clone().into())?)
            .filter(|sys_val| {
                if let SysMetaVal::Create(_) = sys_val {
                    Ok(true)
                } else {
                    Ok(false)
                }
            })
            .count()?
            > 0
        {
            Ok(EntryDhtStatus::Live)
        } else {
            Ok(EntryDhtStatus::Dead)
        }
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!()
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!()
    }
}

mock! {
    pub MetadataBuf
    {
        fn get_links<'a>(&self, key: &'a LinkMetaKey<'a>) -> DatabaseResult<Vec<LinkMetaVal>>;
        fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;
        fn remove_link(&mut self, link_remove: LinkRemove, base: &EntryHash, zome_id: ZomeId, tag: Tag) -> DatabaseResult<()>;
        fn sync_add_create(&self, create: header::EntryCreate) -> DatabaseResult<()>;
        fn sync_add_update(&mut self, update: header::EntryUpdate, entry: Option<EntryHash>) -> DatabaseResult<()>;
        fn sync_add_delete(&self, delete: header::EntryDelete) -> DatabaseResult<()>;
        fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
        fn get_creates(
            &self,
            entry_hash: EntryHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError>>>;
        fn get_updates(
            &self,
            hash: AnyDhtHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError>>>;
        fn get_deletes(
            &self,
            header_hash: HeaderHash,
        ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError>>>;
        }
}

#[async_trait::async_trait]
impl MetadataBufT for MockMetadataBuf {
    fn get_links<'a>(&self, key: &'a LinkMetaKey) -> DatabaseResult<Vec<LinkMetaVal>> {
        self.get_links(key)
    }

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        self.get_canonical_entry_hash(entry_hash)
    }

    fn get_dht_status(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        self.get_dht_status(entry_hash)
    }

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        self.get_canonical_header_hash(header_hash)
    }

    fn get_creates(
        &self,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        self.get_creates(entry_hash)
    }

    fn get_updates(
        &self,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        self.get_updates(hash)
    }

    fn get_deletes(
        &self,
        header_hash: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = SysMetaVal, Error = DatabaseError> + '_>>
    {
        self.get_deletes(header_hash)
    }

    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        self.add_link(link_add)
    }

    fn remove_link(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        zome_id: ZomeId,
        tag: Tag,
    ) -> DatabaseResult<()> {
        self.remove_link(link_remove, base, zome_id, tag)
    }

    async fn add_create(&mut self, create: header::EntryCreate) -> DatabaseResult<()> {
        self.sync_add_create(create)
    }

    async fn add_update(
        &mut self,
        update: header::EntryUpdate,
        entry: Option<EntryHash>,
    ) -> DatabaseResult<()> {
        self.sync_add_update(update, entry)
    }
    async fn add_delete(&mut self, delete: header::EntryDelete) -> DatabaseResult<()> {
        self.sync_add_delete(delete)
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
