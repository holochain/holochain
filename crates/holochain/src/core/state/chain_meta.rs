#![allow(clippy::ptr_arg)]
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{partial_key_match, KvBuf, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use holochain_types::header::{self, builder};
use holochain_types::{
    composite_hash::EntryHash,
    header::{LinkAdd, LinkRemove, ZomeId},
    link::Tag,
    shims::*,
    Header, HeaderHashed, Timestamp,
};
use mockall::mock;
use std::convert::TryInto;
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

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Link {
    pub link_add_hash: HeaderHash,
    pub target: EntryHash,
    pub timestamp: Timestamp,
    pub zome_id: ZomeId,
    pub tag: Tag,
}

/// Key for finding a link
/// Must have add_link_hash for inserts
/// but is optional if you want all links on a get
#[derive(Debug, Clone)]
struct LinkKey<'a> {
    base: &'a EntryHash,
    zome_id: Option<ZomeId>,
    tag: Option<Tag>,
    link_add_hash: Option<HeaderHash>,
}

impl<'a> LinkKey<'a> {
    fn to_key(&self) -> Vec<u8> {
        // Possibly FIXME if this expect is actually not true
        let sb: SerializedBytes = self
            .base
            .try_into()
            .expect("entry addresses don't have the unserialize problem");
        let mut vec: Vec<u8> = sb.bytes().to_vec();
        if let Some(zome_id) = self.zome_id {
            vec.push(zome_id);
        }
        if let Some(ref tag) = self.tag {
            vec.extend_from_slice(tag.as_ref());
        }
        if let Some(ref link_add_hash) = self.link_add_hash {
            vec.extend_from_slice(link_add_hash.as_ref());
        }
        vec
    }
}

impl<'a> From<(&'a LinkAdd, HeaderHash)> for LinkKey<'a> {
    fn from((link_add, hash): (&'a LinkAdd, HeaderHash)) -> Self {
        Self {
            base: &link_add.base_address,
            zome_id: Some(link_add.zome_id),
            tag: Some(link_add.tag.clone()),
            link_add_hash: Some(hash),
        }
    }
}

#[async_trait::async_trait]
pub trait ChainMetaBufT {
    // Links
    /// Get all te links on this base that match the tag
    fn get_links(
        &self,
        base: &EntryHash,
        zome_id: Option<ZomeId>,
        tag: Option<Tag>,
    ) -> DatabaseResult<Vec<Link>>;

    /// Add a link
    async fn add_link<'a>(&'a mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Remove a link
    fn remove_link(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        zome_id: ZomeId,
        tag: Tag,
    ) -> DatabaseResult<()>;

    fn add_update(&self, update: header::EntryUpdate) -> DatabaseResult<HeaderHash>;
    fn add_delete(&self, delete: header::EntryDelete) -> DatabaseResult<HeaderHash>;

    fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {}

pub struct ChainMetaBuf<'env> {
    system_meta: KvvBuf<'env, Vec<u8>, SysMetaVal, Reader<'env>>,
    links_meta: KvBuf<'env, Vec<u8>, Link, Reader<'env>>,
}

impl<'env> ChainMetaBuf<'env> {
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
}

#[async_trait::async_trait]
impl<'env> ChainMetaBufT for ChainMetaBuf<'env> {
    // TODO find out whether we need link_type.
    fn get_links(
        &self,
        base: &EntryHash,
        zome_id: Option<ZomeId>,
        tag: Option<Tag>,
    ) -> DatabaseResult<Vec<Link>> {
        let key = LinkKey {
            base,
            zome_id,
            tag,
            link_add_hash: None,
        };
        debug!(?key);
        let k_bytes = key.to_key();
        // TODO: Internalize this abstraction to KvBuf?
        // TODO: PERF: with_capacity
        let mut links = Vec::new();
        for link in self.links_meta.iter_from(k_bytes.clone())? {
            let (k, link) = link?;
            if partial_key_match(&k_bytes[..], &k) {
                links.push(link)
            } else {
                break;
            }
        }
        Ok(links)
    }

    async fn add_link<'a>(&'a mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        let (link_add, link_add_hash): (Header, HeaderHash) =
            HeaderHashed::with_data(Header::LinkAdd(link_add))
                .await?
                .into();
        let link_add = match link_add {
            Header::LinkAdd(link_add) => link_add,
            _ => unreachable!("BUG: Was hashed as LinkAdd but is not anymore"),
        };
        let key = LinkKey::from((&link_add, link_add_hash.clone()));

        self.links_meta.put(
            key.to_key(),
            Link {
                link_add_hash,
                target: link_add.target_address,
                timestamp: link_add.timestamp,
                zome_id: link_add.zome_id,
                tag: link_add.tag,
            },
        );
        DatabaseResult::Ok(())
    }

    fn remove_link(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        zome_id: ZomeId,
        tag: Tag,
    ) -> DatabaseResult<()> {
        let key = LinkKey {
            base,
            zome_id: Some(zome_id),
            tag: Some(tag),
            link_add_hash: Some(link_remove.link_add_address),
        };
        debug!(removing_key = ?key);
        // TODO: It should be impossible to ever remove a Link that wasn't already added
        // because of the validation dependency on LinkAdd from LinkRemove
        // but do we want some kind of warning or panic here incase we mssed up?
        self.links_meta.delete(key.to_key());
        DatabaseResult::Ok(())
    }

    fn add_update(&self, update: header::EntryUpdate) -> DatabaseResult<HeaderHash> {
        todo!()
    }

    fn add_delete(&self, delete: header::EntryDelete) -> DatabaseResult<HeaderHash> {
        todo!()
    }

    // TODO: remove
    fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        unimplemented!()
    }

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        unimplemented!()
    }

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        unimplemented!()
    }
}

mock! {
    pub ChainMetaBuf
    {
        fn get_links(&self, base: &EntryHash, zome_id: Option<ZomeId>, tag: Option<Tag>) -> DatabaseResult<Vec<Link>>;
        fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;
        fn remove_link(&mut self, link_remove: LinkRemove, base: &EntryHash, zome_id: ZomeId, tag: Tag) -> DatabaseResult<()>;
        fn add_update(&self, update: header::EntryUpdate) -> DatabaseResult<()>;
        fn add_delete(&self, delete: header::EntryDelete) -> DatabaseResult<()>;
        fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
    }
}

#[async_trait::async_trait]
impl ChainMetaBufT for MockChainMetaBuf {
    fn get_links(
        &self,
        base: &EntryHash,
        zome_id: Option<ZomeId>,
        tag: Option<Tag>,
    ) -> DatabaseResult<Vec<Link>> {
        self.get_links(base, zome_id, tag)
    }

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        self.get_canonical_entry_hash(entry_hash)
    }

    fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus> {
        self.get_crud(entry_hash)
    }

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        self.get_canonical_header_hash(header_hash)
    }

    async fn add_link<'a>(&'a mut self, link_add: LinkAdd) -> DatabaseResult<()> {
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

    fn add_update(&self, update: header::EntryUpdate) -> DatabaseResult<HeaderHash> {
        todo!()
    }
    fn add_delete(&self, delete: header::EntryDelete) -> DatabaseResult<HeaderHash> {
        todo!()
    }
}

impl<'env> BufferedStore<'env> for ChainMetaBuf<'env> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        Ok(())
    }
}
