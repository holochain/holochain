#![allow(clippy::ptr_arg)]
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvvBuf,
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use holochain_types::{
    composite_hash::EntryHash,
    header::{EntryDelete, EntryUpdate, LinkAdd},
    shims::*,
};
use mockall::mock;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt::Debug;

mod sys_meta;
pub use sys_meta::*;

type Tag = String;

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

#[allow(dead_code)]
enum Op {
    Add,
    Remove,
}

#[allow(dead_code)]
struct LinkKey<'a> {
    base: &'a EntryHash,
    op: Op,
    tag: Tag,
}

impl<'a> LinkKey<'a> {
    fn to_key(&self) -> Vec<u8> {
        // Possibly FIXME if this expect is actually not true
        let sb: SerializedBytes = self
            .base
            .try_into()
            .expect("entry addresses don't have the unserialize problem");
        let mut vec: Vec<u8> = sb.bytes().to_vec();
        vec.extend_from_slice(self.tag.as_ref());
        vec
    }
}

/*
TODO impliment these types
AddLink:
Base: hash
Target: hash
Type: (maybe?)
Tag: string
Addlink_Time: timestamp
Addlink_Action: hash

RemoveLink:
Base:
Target:
Type:
Tag:
AddLink_Time: timestamp
AddLink_Action: hash
RemoveLink_Action: timestamp
RemoveLink_Action: hash
*/

pub trait ChainMetaBufT<'env, R = Reader<'env>>
where
    R: Readable,
{
    // Links
    /// Get all te links on this base that match the tag
    fn get_links<Tag: Into<String>>(
        &self,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<HashSet<EntryHash>>;

    /// Add a link
    fn add_link(&self, link: LinkAdd) -> DatabaseResult<()>;

    fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()>;
    fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()>;

    fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
}
pub struct ChainMetaBuf<'env, R = Reader<'env>>
where
    R: Readable,
{
    system_meta: KvvBuf<'env, Vec<u8>, SysMetaVal, R>,
    links_meta: KvvBuf<'env, Vec<u8>, LinkMetaVal, R>,
}

impl<'env, R> ChainMetaBuf<'env, R>
where
    R: Readable,
{
    pub(crate) fn new(
        reader: &'env R,
        system_meta: MultiStore,
        links_meta: MultiStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBuf::new(reader, system_meta)?,
            links_meta: KvvBuf::new(reader, links_meta)?,
        })
    }
    pub fn primary(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*PRIMARY_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*PRIMARY_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

    pub fn cache(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for ChainMetaBuf<'env, R>
where
    R: Readable,
{
    // TODO find out whether we need link_type.
    fn get_links<Tag: Into<String>>(
        &self,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<HashSet<EntryHash>> {
        // TODO get removes
        // TODO get adds
        let key = LinkKey {
            op: Op::Add,
            base,
            tag: tag.into(),
        };
        let _values = self.links_meta.get(&key.to_key());
        Ok(HashSet::new())
    }

    fn add_link(&self, link: LinkAdd) -> DatabaseResult<()> {
        todo!()
    }

    fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()> {
        todo!()
    }
    fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()> {
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
        fn get_links(&self, base: &EntryHash, tag: Tag) -> DatabaseResult<HashSet<EntryHash>>;
        fn add_link(&self, link: LinkAdd) -> DatabaseResult<()>;
        fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()>;
        fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()>;
        fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for MockChainMetaBuf
where
    R: Readable,
{
    fn get_links<Tag: Into<String>>(
        &self,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<HashSet<EntryHash>> {
        self.get_links(base, tag.into())
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

    fn add_link(&self, link: LinkAdd) -> DatabaseResult<()> {
        self.add_link(link)
    }

    fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()> {
        self.add_update(update)
    }
    fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()> {
        self.add_delete(delete)
    }
}

impl<'env, R: Readable> BufferedStore<'env> for ChainMetaBuf<'env, R> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fixt::EntryFixturator;
    use fixt::prelude::*;
    use holo_hash::{AgentPubKeyFixturator, HeaderHashFixturator};
    use holochain_state::{buffer::BufferedStore, test_utils::test_cell_env};
    use holochain_types::{EntryHashed, Timestamp};
    use maplit::hashset;

    #[tokio::test(threaded_scheduler)]
    async fn can_add_and_get_link() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        // why the block_on here, when we're already in an async fn?
        let (base_hash, target_hash) = tokio_safe_block_on::tokio_safe_block_on(
            async {
                let mut entry_fix = EntryFixturator::new(Unpredictable);
                (
                    EntryHashed::with_data(entry_fix.next().unwrap())
                        .await
                        .unwrap(),
                    EntryHashed::with_data(entry_fix.next().unwrap())
                        .await
                        .unwrap(),
                )
            },
            std::time::Duration::from_secs(1),
        )
        .unwrap();

        let mut ser_fix = SerializedBytesFixturator::new(Unpredictable);
        let base_address: &EntryHash = base_hash.as_ref();
        let target_address: &EntryHash = target_hash.as_ref();
        let add_link = LinkAdd {
            author: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: 0,
            prev_header: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
            base_address: base_address.clone(),
            target_address: target_address.clone(),
            tag: ser_fix.next().unwrap(),
            link_type: ser_fix.next().unwrap(),
        };

        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert!(meta_buf
                .get_links(base_hash.as_ref(), "")
                .unwrap()
                .is_empty());
            DatabaseResult::Ok(())
        })
        .unwrap();

        env.with_commit(|writer| {
            let reader = env.reader().unwrap();
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            meta_buf.add_link(add_link).unwrap();
            meta_buf.flush_to_txn(writer)
        })
        .unwrap();

        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert_eq!(
                meta_buf.get_links(base_hash.as_ref(), "").unwrap(),
                hashset! {target_address.clone()}
            );
            DatabaseResult::Ok(())
        })
        .unwrap();
    }
}
