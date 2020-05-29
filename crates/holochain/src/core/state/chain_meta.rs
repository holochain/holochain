#![allow(clippy::ptr_arg)]
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{partial_key_match, KvBuf, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::{DatabaseError, DatabaseResult},
    prelude::*,
};
use holochain_types::header::{EntryDelete, EntryUpdate};
use holochain_types::{
    composite_hash::EntryHash,
    header::{LinkAdd, LinkRemove},
    shims::*,
    Header, HeaderHashed,
};
use mockall::mock;
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

// TODO Add Op to value
// Adds have hash of LinkAdd
// And target
// Dels have hash of LinkAdd
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
enum Op {
    Add(HeaderHash, EntryHash),
    Remove(HeaderHash),
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
struct Link {
    link_add_hash: HeaderHash,
    target: EntryHash,
}

/// Key for finding a link
/// Must have add_link_hash for inserts
/// but is optional if you want all links on a get
struct LinkKey<'a> {
    base: &'a EntryHash,
    tag: Tag,
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
        vec.extend_from_slice(self.tag.as_ref());
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
            tag: link_add.tag.clone(),
            link_add_hash: Some(hash),
        }
    }
}

#[async_trait::async_trait]
pub trait ChainMetaBufT {
    // Links
    /// Get all te links on this base that match the tag
    fn get_links<Tag>(&self, base: &EntryHash, tag: Tag) -> DatabaseResult<Vec<EntryHash>>
    where
        Tag: Into<String>;

    /// Add a link
    async fn add_link<'a>(&'a mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Remove a link
    fn remove_link<Tag: Into<String>>(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<()>;

    fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()>;
    fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()>;

    fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;

    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
}

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
    fn get_links<Tag: Into<String>>(
        &self,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<Vec<EntryHash>> {
        let key = LinkKey {
            base,
            tag: tag.into(),
            link_add_hash: None,
        };
        let k_bytes = key.to_key();
        // TODO: Internalizethis abstraction to KvBuf
        // TODO: PERF: with_capacity
        let mut links = Vec::new();
        for link in self.links_meta.iter_from(k_bytes.clone())? {
            let (k, link) = link?;
            if partial_key_match(&k_bytes[..], &k) {
                links.push(link.target)
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
            },
        );
        DatabaseResult::Ok(())
    }

    fn remove_link<Tag: Into<String>>(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<()> {
        let key = LinkKey {
            base,
            tag: tag.into(),
            link_add_hash: Some(link_remove.link_add_address),
        };
        // TODO: It should be impossible to ever remove a Link that wasn't already added
        // because of the validation dependency on LinkAdd from LinkRemove
        // but do we want some kind of warning or panic here incase we mssed up?
        self.links_meta.delete(key.to_key());
        DatabaseResult::Ok(())
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
        fn get_links(&self, base: &EntryHash, tag: Tag) -> DatabaseResult<Vec<EntryHash>>;
        fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;
        fn remove_link(&mut self, link_remove: LinkRemove, base: &EntryHash, tag: Tag) -> DatabaseResult<()>;
        fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()>;
        fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()>;
        fn get_crud(&self, entry_hash: &EntryHash) -> DatabaseResult<EntryDhtStatus>;
        fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;
        fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;
    }
}

#[async_trait::async_trait]
impl ChainMetaBufT for MockChainMetaBuf {
    fn get_links<Tag: Into<String>>(
        &self,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<Vec<EntryHash>> {
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

    async fn add_link<'a>(&'a mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        self.add_link(link_add)
    }

    fn remove_link<Tag: Into<String>>(
        &mut self,
        link_remove: LinkRemove,
        base: &EntryHash,
        tag: Tag,
    ) -> DatabaseResult<()> {
        self.remove_link(link_remove, base, tag.into())
    }

    fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()> {
        self.add_update(update)
    }
    fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()> {
        self.add_delete(delete)
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::fixt::EntryFixturator;
    use fixt::prelude::*;
    use holo_hash::{
        AgentPubKeyFixturator, EntryContentHashFixturator, Hashable, HeaderHashFixturator,
    };
    use holochain_state::{buffer::BufferedStore, test_utils::test_cell_env};
    use holochain_types::{EntryHashed, Timestamp};

    fixturator!(
        LinkAdd;
        curve Empty LinkAdd {
            author: AgentPubKeyFixturator::new(Empty).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Empty).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Empty).next().unwrap(),
            base_address: EntryContentHashFixturator::new(Empty).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new(Empty).next().unwrap().into(),
            tag: StringFixturator::new(Empty).next().unwrap(),
            link_type: SerializedBytesFixturator::new(Empty).next().unwrap(),
        };
        curve Unpredictable LinkAdd {
            author: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Unpredictable).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
            base_address: EntryContentHashFixturator::new(Unpredictable).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new(Unpredictable).next().unwrap().into(),
            tag: StringFixturator::new(Unpredictable).next().unwrap(),
            link_type: SerializedBytesFixturator::new(Unpredictable).next().unwrap(),
        };
        curve Predictable LinkAdd {
            author: AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            prev_header: HeaderHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            base_address: EntryContentHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into(),
            tag: StringFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            link_type: SerializedBytesFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
        };
    );

    fixturator!(
        LinkRemove;
        curve Empty LinkRemove {
            author: AgentPubKeyFixturator::new(Empty).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Empty).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Empty).next().unwrap(),
            link_add_address: HeaderHashFixturator::new(Empty).next().unwrap(),
        };
        curve Unpredictable LinkRemove {
            author: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Unpredictable).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
            link_add_address: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
        };
        curve Predictable LinkRemove {
            author: AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            prev_header: HeaderHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            link_add_address: HeaderHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
        };
    );

    struct KnownLinkAdd {
        base_address: EntryHash,
        target_address: EntryHash,
        tag: String,
    }

    struct KnownLinkRemove {
        link_add_address: HeaderHash,
    }

    impl Iterator for LinkAddFixturator<KnownLinkAdd> {
        type Item = LinkAdd;
        fn next(&mut self) -> Option<Self::Item> {
            let mut f = LinkAddFixturator::new(Unpredictable).next().unwrap();
            f.base_address = self.0.curve.base_address.clone();
            f.target_address = self.0.curve.target_address.clone();
            f.tag = self.0.curve.tag.clone();
            Some(f)
        }
    }

    impl Iterator for LinkRemoveFixturator<KnownLinkRemove> {
        type Item = LinkRemove;
        fn next(&mut self) -> Option<Self::Item> {
            let mut f = LinkRemoveFixturator::new(Unpredictable).next().unwrap();
            f.link_add_address = self.0.curve.link_add_address.clone();
            Some(f)
        }
    }

    async fn entries() -> (EntryHashed, EntryHashed) {
        let mut entry_fix = EntryFixturator::new(Unpredictable);
        (
            EntryHashed::with_data(entry_fix.next().unwrap())
                .await
                .unwrap(),
            EntryHashed::with_data(entry_fix.next().unwrap())
                .await
                .unwrap(),
        )
    }

    #[tokio::test(threaded_scheduler)]
    async fn can_add_and_get_link() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        // Create a known link add
        let (base_hash, target_hash) = entries().await;

        let tag = StringFixturator::new(Unpredictable).next().unwrap();
        let base_address: &EntryHash = base_hash.as_ref();
        let target_address: &EntryHash = target_hash.as_ref();

        let link_add = KnownLinkAdd {
            base_address: base_address.clone(),
            target_address: target_address.clone(),
            tag: tag.clone(),
        };

        let link_add = LinkAddFixturator::new(link_add).next().unwrap();

        // Check it's empty
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert!(meta_buf
                .get_links(base_hash.as_ref(), tag.clone())
                .unwrap()
                .is_empty());
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Add a link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            meta_buf.add_link(link_add).await.unwrap();
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check it's there
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert_eq!(
                meta_buf.get_links(base_hash.as_ref(), tag.clone()).unwrap(),
                vec![target_address.clone()]
            );
            DatabaseResult::Ok(())
        })
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn can_add_and_remove_link() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        // Create a known link add
        let (base_hash, target_hash) = entries().await;

        let tag = StringFixturator::new(Unpredictable).next().unwrap();
        let base_address: &EntryHash = base_hash.as_ref();
        let target_address: &EntryHash = target_hash.as_ref();

        let link_add = KnownLinkAdd {
            base_address: base_address.clone(),
            target_address: target_address.clone(),
            tag: tag.clone(),
        };

        let link_add = LinkAddFixturator::new(link_add).next().unwrap();

        // Create a known link remove
        let link_add_address = HeaderHashed::with_data(Header::LinkAdd(link_add.clone()))
            .await
            .unwrap();
        let link_add_address: &HeaderHash = link_add_address.as_ref();
        let link_remove = KnownLinkRemove {
            link_add_address: link_add_address.clone(),
        };
        let link_remove = LinkRemoveFixturator::new(link_remove).next().unwrap();

        // Check it's empty
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert!(meta_buf
                .get_links(base_hash.as_ref(), tag.clone())
                .unwrap()
                .is_empty());
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Add a link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            meta_buf.add_link(link_add).await.unwrap();
            assert_eq!(
                meta_buf.get_links(base_hash.as_ref(), tag.clone()).unwrap(),
                vec![target_address.clone()]
            );
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check it's there
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert_eq!(
                meta_buf.get_links(base_hash.as_ref(), tag.clone()).unwrap(),
                vec![target_address.clone()]
            );
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Remove the link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            meta_buf
                .remove_link(link_remove, base_address, tag.clone())
                .unwrap();
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check it's empty
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            assert!(meta_buf
                .get_links(base_hash.as_ref(), tag.clone())
                .unwrap()
                .is_empty());
            DatabaseResult::Ok(())
        })
        .unwrap();
    }
}
