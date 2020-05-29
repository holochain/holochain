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
    header::{LinkAdd, LinkRemove, ZomeId},
    link::Tag,
    shims::*,
    Header, HeaderHashed, Timestamp,
};
use mockall::mock;
use std::convert::TryInto;
use std::fmt::Debug;

mod sys_meta;
pub use sys_meta::*;
use tracing::*;

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
        fn get_links(&self, base: &EntryHash, zome_id: Option<ZomeId>, tag: Option<Tag>) -> DatabaseResult<Vec<Link>>;
        fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;
        fn remove_link(&mut self, link_remove: LinkRemove, base: &EntryHash, zome_id: ZomeId, tag: Tag) -> DatabaseResult<()>;
        fn add_update(&self, update: EntryUpdate) -> DatabaseResult<()>;
        fn add_delete(&self, delete: EntryDelete) -> DatabaseResult<()>;
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
pub mod test {
    use super::*;
    use crate::fixt::EntryFixturator;
    use fixt::prelude::*;
    use holo_hash::{
        AgentPubKeyFixturator, EntryContentHashFixturator, Hashable, HeaderHashFixturator,
    };
    use holochain_state::{buffer::BufferedStore, test_utils::test_cell_env};
    use holochain_types::{observability, EntryHashed, Timestamp};

    fixturator!(
        LinkAdd;
        curve Empty LinkAdd {
            author: AgentPubKeyFixturator::new(Empty).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Empty).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Empty).next().unwrap(),
            base_address: EntryContentHashFixturator::new(Empty).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new(Empty).next().unwrap().into(),
            zome_id: U8Fixturator::new(Empty).next().unwrap(),
            tag: Tag::new(BytesFixturator::new(Empty).next().unwrap()),
        };
        curve Unpredictable LinkAdd {
            author: AgentPubKeyFixturator::new(Unpredictable).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new(Unpredictable).next().unwrap(),
            prev_header: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
            base_address: EntryContentHashFixturator::new(Unpredictable).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new(Unpredictable).next().unwrap().into(),
            zome_id: U8Fixturator::new(Unpredictable).next().unwrap(),
            tag: Tag::new(BytesFixturator::new(Unpredictable).next().unwrap()),
        };
        curve Predictable LinkAdd {
            author: AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            timestamp: Timestamp::now(),
            header_seq: U32Fixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            prev_header: HeaderHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            base_address: EntryContentHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into(),
            target_address: EntryContentHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into(),
            zome_id: U8Fixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            tag: Tag::new(BytesFixturator::new_indexed(Predictable, self.0.index).next().unwrap()),
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

    fixturator!(
        Link;
        curve Empty Link {
            timestamp: Timestamp::now(),
            link_add_hash: HeaderHashFixturator::new(Empty).next().unwrap(),
            target: EntryContentHashFixturator::new(Empty).next().unwrap().into(),
            zome_id: U8Fixturator::new(Empty).next().unwrap(),
            tag: Tag::new(BytesFixturator::new(Empty).next().unwrap()),
        };
        curve Unpredictable Link {
            timestamp: Timestamp::now(),
            link_add_hash: HeaderHashFixturator::new(Unpredictable).next().unwrap(),
            target: EntryContentHashFixturator::new(Unpredictable).next().unwrap().into(),
            zome_id: U8Fixturator::new(Unpredictable).next().unwrap(),
            tag: Tag::new(BytesFixturator::new(Unpredictable).next().unwrap()),
        };
        curve Predictable Link {
            timestamp: Timestamp::now(),
            link_add_hash: HeaderHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            target: EntryContentHashFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into(),
            zome_id: U8Fixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            tag: Tag::new(BytesFixturator::new_indexed(Predictable, self.0.index).next().unwrap()),
        };
    );

    pub struct KnownLinkAdd {
        base_address: EntryHash,
        target_address: EntryHash,
        tag: Tag,
        zome_id: ZomeId,
    }

    pub struct KnownLinkRemove {
        link_add_address: HeaderHash,
    }

    impl Iterator for LinkAddFixturator<KnownLinkAdd> {
        type Item = LinkAdd;
        fn next(&mut self) -> Option<Self::Item> {
            let mut f = LinkAddFixturator::new(Unpredictable).next().unwrap();
            f.base_address = self.0.curve.base_address.clone();
            f.target_address = self.0.curve.target_address.clone();
            f.tag = self.0.curve.tag.clone();
            f.zome_id = self.0.curve.zome_id.clone();
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

    impl Iterator for LinkFixturator<(EntryHash, Tag)> {
        type Item = Link;
        fn next(&mut self) -> Option<Self::Item> {
            let mut f = LinkFixturator::new(Unpredictable).next().unwrap();
            f.target = self.0.curve.0.clone();
            f.tag = self.0.curve.1.clone();
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

    macro_rules! here {
        ($test: expr) => {
            concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
        };
    }

    struct TestData {
        link_add: LinkAdd,
        link_remove: LinkRemove,
        base_hash: EntryHash,
        zome_id: ZomeId,
        tag: Tag,
        expected_link: Link,
    }

    async fn fixtures(n: usize) -> Vec<TestData> {
        let data = (0..n).map(|_| async {
            // Create a known link add
            let (base_hash, target_hash) = entries().await;

            let tag = Tag::new(BytesFixturator::new(Unpredictable).next().unwrap());
            let zome_id = U8Fixturator::new(Unpredictable).next().unwrap();
            let base_address: &EntryHash = base_hash.as_ref();
            let target_address: &EntryHash = target_hash.as_ref();

            let link_add = KnownLinkAdd {
                base_address: base_address.clone(),
                target_address: target_address.clone(),
                zome_id,
                tag: tag.clone(),
            };

            let link_add = LinkAddFixturator::new(link_add).next().unwrap();

            // Create the expected link result
            let (_, link_add_hash): (_, HeaderHash) =
                HeaderHashed::with_data(Header::LinkAdd(link_add.clone()))
                    .await
                    .unwrap()
                    .into();

            let expected_link = Link {
                link_add_hash: link_add_hash.clone(),
                target: target_address.clone(),
                timestamp: link_add.timestamp.clone(),
                zome_id,
                tag: tag.clone(),
            };

            let link_remove = KnownLinkRemove {
                link_add_address: link_add_hash,
            };
            let link_remove = LinkRemoveFixturator::new(link_remove).next().unwrap();
            TestData {
                link_add,
                link_remove,
                base_hash: base_address.clone(),
                zome_id,
                tag,
                expected_link,
            }
        });
        futures::future::join_all(data).await.into()
    }

    impl TestData {
        fn empty(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            assert!(
                meta_buf
                    .get_links(&self.base_hash, Some(self.zome_id), Some(self.tag.clone()))
                    .unwrap()
                    .is_empty(),
                test
            );
        }

        fn present(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            assert_eq!(
                &meta_buf
                    .get_links(&self.base_hash, Some(self.zome_id), Some(self.tag.clone()))
                    .unwrap()[..],
                &[self.expected_link.clone()],
                "{}",
                test
            );
        }

        fn not_present(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            assert!(
                !meta_buf
                    .get_links(&self.base_hash, Some(self.zome_id), Some(self.tag.clone()))
                    .unwrap()
                    .contains(&self.expected_link),
                "Link: {:?} should not be present {}",
                self.expected_link,
                test
            );
        }

        fn base(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            assert_eq!(
                &meta_buf.get_links(&self.base_hash, None, None).unwrap()[..],
                &[self.expected_link.clone()],
                "{}",
                test
            );
        }

        fn zome_id(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            assert_eq!(
                &meta_buf
                    .get_links(&self.base_hash, Some(self.zome_id), None)
                    .unwrap()[..],
                &[self.expected_link.clone()],
                "{}",
                test
            );
        }

        fn half_tag(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
            let tag_len = self.tag.len();
            let half_tag = tag_len / 2;
            let half_tag = Tag::new(&self.tag[..half_tag]);
            assert_eq!(
                &meta_buf
                    .get_links(&self.base_hash, Some(self.zome_id), Some(half_tag))
                    .unwrap()[..],
                &[self.expected_link.clone()],
                "{}",
                test
            );
        }

        async fn add_link(&self, meta_buf: &mut ChainMetaBuf<'_>) {
            meta_buf.add_link(self.link_add.clone()).await.unwrap();
        }
        async fn remove_link(&self, meta_buf: &mut ChainMetaBuf<'_>) {
            meta_buf
                .remove_link(
                    self.link_remove.clone(),
                    &self.base_hash,
                    self.zome_id,
                    self.tag.clone(),
                )
                .unwrap();
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn can_add_and_remove_link() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        let td = fixtures(1).await.into_iter().next().unwrap();

        // Check it's empty
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            td.empty(here!("empty at start"), &meta_buf);
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Add a link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            td.add_link(&mut meta_buf).await;
            // Is in scratch
            td.present(here!("add link in scratch"), &meta_buf);

            // Remove from scratch
            td.remove_link(&mut meta_buf).await;

            // Is empty
            td.empty(here!("empty after remove"), &meta_buf);

            // Add again
            td.add_link(&mut meta_buf).await;

            // Is in scratch again
            td.present(here!("Is still in the scratch"), &meta_buf);

            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check it's in db
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            td.present(here!("It's in the db"), &meta_buf);
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Remove the link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            td.remove_link(&mut meta_buf).await;
            // Is empty
            td.empty(here!("empty after remove"), &meta_buf);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Check it's empty
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Is empty
            td.empty(here!("empty after remove in db"), &meta_buf);
            DatabaseResult::Ok(())
        })
        .unwrap();

        // Add a link
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            td.add_link(&mut meta_buf).await;
            // Is in scratch
            td.present(here!("add link in scratch"), &meta_buf);
            // No zome, no tag
            td.base(here!("scratch"), &meta_buf);
            // No tag
            td.zome_id(here!("scratch"), &meta_buf);
            // Half the tag
            td.half_tag(here!("scratch"), &meta_buf);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        // Partial matching
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            td.present(here!("db"), &meta_buf);
            // No zome, no tag
            td.base(here!("db"), &meta_buf);
            // No tag
            td.zome_id(here!("db"), &meta_buf);
            // Half the tag
            td.half_tag(here!("db"), &meta_buf);
            DatabaseResult::Ok(())
        })
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn multiple_links() {
        let arc = test_cell_env();
        let env = arc.guard().await;

        let td = fixtures(10).await;

        // Add links
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            for d in td.iter() {
                d.add_link(&mut meta_buf).await;
            }
            // Is in scratch
            for d in td.iter() {
                d.present(here!("add link in scratch"), &meta_buf);
            }

            // Remove from scratch
            td[5].remove_link(&mut meta_buf).await;

            td[5].not_present(here!("removed in scratch"), &meta_buf);

            for d in td[0..5].iter().chain(&td[6..]) {
                d.present(here!("all except 5 scratch"), &meta_buf);
            }
            // Add again
            td[5].add_link(&mut meta_buf).await;

            // Is in scratch again
            td[5].present(here!("Is back in the scratch"), &meta_buf);

            for d in td.iter() {
                d.present(here!("add link in scratch"), &meta_buf);
            }

            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            for d in td.iter() {
                d.present(here!("all in db"), &meta_buf);
            }
            td[0].remove_link(&mut meta_buf).await;

            for d in &td[1..] {
                d.present(here!("all except 0 scratch"), &meta_buf);
            }

            td[0].not_present(here!("removed in scratch"), &meta_buf);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }

        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            for d in &td[1..] {
                d.present(here!("all except 0"), &meta_buf);
            }
            td[0].not_present(here!("removed in db"), &meta_buf);
            DatabaseResult::Ok(())
        })
        .unwrap();
    }
    #[tokio::test(threaded_scheduler)]
    async fn duplicate_links() {
        observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;

        let td = fixtures(10).await;
        // Add to db then the same to scratch and expect on one result
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            for d in td.iter() {
                d.add_link(&mut meta_buf).await;
            }
            // Is in scratch
            for d in td.iter() {
                d.present(here!("re add"), &meta_buf);
                // No zome, no tag
                d.base(here!("re add"), &meta_buf);
                // No tag
                d.zome_id(here!("re add"), &meta_buf);
                // Half the tag
                d.half_tag(here!("re add"), &meta_buf);
            }
            // Add Again
            for d in td.iter() {
                d.add_link(&mut meta_buf).await;
            }
            // Is in scratch
            for d in td.iter() {
                d.present(here!("re add"), &meta_buf);
                // No zome, no tag
                d.base(here!("re add"), &meta_buf);
                // No tag
                d.zome_id(here!("re add"), &meta_buf);
                // Half the tag
                d.half_tag(here!("re add"), &meta_buf);
            }
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            for d in td.iter() {
                d.add_link(&mut meta_buf).await;
            }
            // Is in scratch
            for d in td.iter() {
                d.present(here!("re add"), &meta_buf);
                // No zome, no tag
                d.base(here!("re add"), &meta_buf);
                // No tag
                d.zome_id(here!("re add"), &meta_buf);
                // Half the tag
                d.half_tag(here!("re add"), &meta_buf);
            }
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Is in db
            for d in td.iter() {
                d.present(here!("re add"), &meta_buf);
                // No zome, no tag
                d.base(here!("re add"), &meta_buf);
                // No tag
                d.zome_id(here!("re add"), &meta_buf);
                // Half the tag
                d.half_tag(here!("re add"), &meta_buf);
            }
            DatabaseResult::Ok(())
        })
        .unwrap();
    }
    #[tokio::test(threaded_scheduler)]
    async fn links_on_same_base() {
        observability::test_run().ok();
        let arc = test_cell_env();
        let env = arc.guard().await;

        let mut td = fixtures(10).await;
        let base_hash = td[0].base_hash.clone();
        let base_hash = &base_hash;
        for d in td.iter_mut() {
            d.base_hash = base_hash.clone();
            d.link_add.base_address = base_hash.clone();
            // Create the new hash
            let (_, link_add_hash): (_, HeaderHash) =
                HeaderHashed::with_data(Header::LinkAdd(d.link_add.clone()))
                    .await
                    .unwrap()
                    .into();
            d.expected_link.link_add_hash = link_add_hash.clone();
            d.link_remove.link_add_address = link_add_hash;
        }
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // Add
            for d in td.iter() {
                d.add_link(&mut meta_buf).await;
            }
            // Is in scratch
            for d in td.iter() {
                d.present(here!("same base"), &meta_buf);
                // No tag
                // FIXME: This test is failing because the zome_ids, aren't unique
                d.zome_id(here!("same base"), &meta_buf);
                // Half the tag
                d.half_tag(here!("same base"), &meta_buf);
            }
            // TODO: Check they all return off the same base
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        env.with_reader(|reader| {
            let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
            // In db
            for d in td.iter() {
                d.present(here!("same base"), &meta_buf);
                // No tag
                d.zome_id(here!("same base"), &meta_buf);
                // Half the tag
                d.half_tag(here!("same base"), &meta_buf);
            }
            // TODO: Check they all return off the same base
            DatabaseResult::Ok(())
        })
        .unwrap();
        // TODO Check removes etc.
    }
}
