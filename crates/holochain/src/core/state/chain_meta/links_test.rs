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

fixturator!(
    EntryHash;
    variants [
        Entry(EntryContentHash)
        Agent(AgentPubKey)
    ];
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

macro_rules! here {
    ($test: expr) => {
        concat!($test, " !!!_LOOK HERE:---> ", file!(), ":", line!())
    };
}

#[derive(Clone)]
struct TestData {
    link_add: LinkAdd,
    link_remove: LinkRemove,
    base_hash: EntryHash,
    zome_id: ZomeId,
    tag: Tag,
    expected_link: Link,
}

async fn fixtures(n: usize) -> Vec<TestData> {
    let mut tag_fix = BytesFixturator::new(Predictable);
    let mut zome_id = U8Fixturator::new(Predictable);
    let mut data = Vec::new();
    let mut base_hash_fixt = EntryHashFixturator::new(Predictable);
    let mut target_hash_fixt = EntryHashFixturator::new(Unpredictable);
    for _ in 0..n {
        // Create a known link add
        let base_address = base_hash_fixt.next().unwrap();
        let target_address = target_hash_fixt.next().unwrap();

        let tag = Tag::new(tag_fix.next().unwrap());
        let zome_id = zome_id.next().unwrap();

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
        let td = TestData {
            link_add,
            link_remove,
            base_hash: base_address.clone(),
            zome_id,
            tag,
            expected_link,
        };
        data.push(td);
    }
    data
}

#[allow(dead_code)]
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

    fn is_on_full_key(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert!(
            meta_buf
                .get_links(&self.base_hash, Some(self.zome_id), Some(self.tag.clone()))
                .unwrap()
                .contains(&self.expected_link),
            "{}",
            test
        );
    }

    fn only_on_full_key(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert_eq!(
            &meta_buf
                .get_links(&self.base_hash, Some(self.zome_id), Some(self.tag.clone()))
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        );
    }

    fn not_on_full_key(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
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

    fn is_on_base(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert!(
            meta_buf
                .get_links(&self.base_hash, None, None)
                .unwrap()
                .contains(&self.expected_link),
            "Results should contain Link: {:?} in test: {}",
            self.expected_link,
            test
        );
    }

    fn only_on_base(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert_eq!(
            &meta_buf.get_links(&self.base_hash, None, None).unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        );
    }

    fn is_on_zome_id(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert!(
            meta_buf
                .get_links(&self.base_hash, Some(self.zome_id), None)
                .unwrap()
                .contains(&self.expected_link),
            "Results should contain Link: {:?} in test: {}",
            self.expected_link,
            test
        );
    }

    fn only_on_zome_id(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        assert_eq!(
            &meta_buf
                .get_links(&self.base_hash, Some(self.zome_id), None)
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        );
    }

    fn only_on_half_tag(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        let tag_len = self.tag.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
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

    fn is_on_half_tag(&self, test: &'static str, meta_buf: &ChainMetaBuf) {
        let tag_len = self.tag.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = Tag::new(&self.tag[..half_tag]);
        assert!(
            meta_buf
                .get_links(&self.base_hash, Some(self.zome_id), Some(half_tag))
                .unwrap()
                .contains(&self.expected_link),
            "Results should contain Link: {:?} in test: {}",
            self.expected_link,
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

    #[instrument(skip(td, meta_buf))]
    fn only_these_on_base(td: &[Self], test: &'static str, meta_buf: &ChainMetaBuf) {
        // Check all base hash are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| LinkKey::from((&d.0, d.1.link_add_hash.clone())).to_key());
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        assert_eq!(
            &meta_buf.get_links(base_hash, None, None).unwrap()[..],
            &expected[..],
            "{}",
            test
        );
    }

    fn only_these_on_zome_id(td: &[Self], test: &'static str, meta_buf: &ChainMetaBuf) {
        // Check all base hash, zome_id are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
            assert_eq!(d.zome_id, td[0].zome_id, "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let zome_id = td[0].zome_id;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| LinkKey::from((&d.0, d.1.link_add_hash.clone())).to_key());
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        assert_eq!(
            &meta_buf.get_links(base_hash, Some(zome_id), None).unwrap()[..],
            &expected[..],
            "{}",
            test
        );
    }

    fn only_these_on_full_key(td: &[Self], test: &'static str, meta_buf: &ChainMetaBuf) {
        // Check all base hash, zome_id, tag are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
            assert_eq!(d.zome_id, td[0].zome_id, "{}", test);
            assert_eq!(d.tag, td[0].tag, "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let zome_id = td[0].zome_id;
        let tag = &td[0].tag;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| LinkKey::from((&d.0, d.1.link_add_hash.clone())).to_key());
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        assert_eq!(
            &meta_buf
                .get_links(base_hash, Some(zome_id), Some(tag.clone()))
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        );
    }

    fn only_these_on_half_key(td: &[Self], test: &'static str, meta_buf: &ChainMetaBuf) {
        let tag_len = td[0].tag.len();
        // Make sure there is at least some tag
        let tag_len = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = Tag::new(&td[0].tag[..tag_len]);
        // Check all base hash, zome_id, half tag are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
            assert_eq!(d.zome_id, td[0].zome_id, "{}", test);
            assert_eq!(&d.tag[..tag_len], &half_tag[..], "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let zome_id = td[0].zome_id;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| LinkKey::from((&d.0, d.1.link_add_hash.clone())).to_key());
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        assert_eq!(
            &meta_buf
                .get_links(base_hash, Some(zome_id), Some(half_tag))
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        );
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
        td.only_on_full_key(here!("add link in scratch"), &meta_buf);

        // Remove from scratch
        td.remove_link(&mut meta_buf).await;

        // Is empty
        td.empty(here!("empty after remove"), &meta_buf);

        // Add again
        td.add_link(&mut meta_buf).await;

        // Is in scratch again
        td.only_on_full_key(here!("Is still in the scratch"), &meta_buf);

        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    // Check it's in db
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        td.only_on_full_key(here!("It's in the db"), &meta_buf);
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
        td.only_on_full_key(here!("add link in scratch"), &meta_buf);
        // No zome, no tag
        td.only_on_base(here!("scratch"), &meta_buf);
        // No tag
        td.only_on_zome_id(here!("scratch"), &meta_buf);
        // Half the tag
        td.only_on_half_tag(here!("scratch"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    // Partial matching
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        td.only_on_full_key(here!("db"), &meta_buf);
        // No zome, no tag
        td.only_on_base(here!("db"), &meta_buf);
        // No tag
        td.only_on_zome_id(here!("db"), &meta_buf);
        // Half the tag
        td.only_on_half_tag(here!("db"), &meta_buf);
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
            d.only_on_full_key(here!("add link in scratch"), &meta_buf);
        }

        // Remove from scratch
        td[5].remove_link(&mut meta_buf).await;

        td[5].not_on_full_key(here!("removed in scratch"), &meta_buf);

        for d in td[0..5].iter().chain(&td[6..]) {
            d.only_on_full_key(here!("all except 5 scratch"), &meta_buf);
        }
        // Add again
        td[5].add_link(&mut meta_buf).await;

        // Is in scratch again
        td[5].only_on_full_key(here!("Is back in the scratch"), &meta_buf);

        for d in td.iter() {
            d.only_on_full_key(here!("add link in scratch"), &meta_buf);
        }

        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    {
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        for d in td.iter() {
            d.only_on_full_key(here!("all in db"), &meta_buf);
        }
        td[0].remove_link(&mut meta_buf).await;

        for d in &td[1..] {
            d.only_on_full_key(here!("all except 0 scratch"), &meta_buf);
        }

        td[0].not_on_full_key(here!("removed in scratch"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        for d in &td[1..] {
            d.only_on_full_key(here!("all except 0"), &meta_buf);
        }
        td[0].not_on_full_key(here!("removed in db"), &meta_buf);
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
            d.only_on_full_key(here!("re add"), &meta_buf);
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf);
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf);
        }
        // Add Again
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("re add"), &meta_buf);
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf);
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf);
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
            d.only_on_full_key(here!("re add"), &meta_buf);
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf);
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf);
        }
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // Is in db
        for d in td.iter() {
            d.only_on_full_key(here!("re add"), &meta_buf);
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf);
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf);
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
            d.only_on_full_key(here!("same base"), &meta_buf);
            d.only_on_zome_id(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // In db
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf);
            d.only_on_zome_id(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        DatabaseResult::Ok(())
    })
    .unwrap();
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        td[0].remove_link(&mut meta_buf).await;
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"), &meta_buf);
            d.only_on_zome_id(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"), &meta_buf);
        td[0].not_on_full_key(here!("removed in scratch"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"), &meta_buf);
            d.only_on_zome_id(here!("same base"), &meta_buf);
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"), &meta_buf);
        td[0].not_on_full_key(here!("removed in scratch"), &meta_buf);
        DatabaseResult::Ok(())
    })
    .unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn links_on_same_zome_id() {
    observability::test_run().ok();
    let arc = test_cell_env();
    let env = arc.guard().await;

    let mut td = fixtures(10).await;
    let base_hash = td[0].base_hash.clone();
    let zome_id = td[0].zome_id;
    let base_hash = &base_hash;
    for d in td.iter_mut() {
        d.base_hash = base_hash.clone();
        d.zome_id = zome_id;
        d.link_add.base_address = base_hash.clone();
        d.link_add.zome_id = zome_id;
        // Create the new hash
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::with_data(Header::LinkAdd(d.link_add.clone()))
                .await
                .unwrap()
                .into();
        d.expected_link.link_add_hash = link_add_hash.clone();
        d.expected_link.zome_id = zome_id;
        d.link_remove.link_add_address = link_add_hash;
    }
    {
        let span = debug_span!("check_zome_id");
        let _g = span.enter();
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td, here!("check all return on same base"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // In db
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td, here!("check all return on same base"), &meta_buf);
        DatabaseResult::Ok(())
    })
    .unwrap();
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        td[9].remove_link(&mut meta_buf).await;
        for d in &td[..9] {
            d.only_on_full_key(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td[..9], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(
            &td[..9],
            here!("check all return on same base"),
            &meta_buf,
        );
        td[9].not_on_full_key(here!("removed in scratch"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        for d in &td[..9] {
            d.only_on_full_key(here!("same base"), &meta_buf);
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf);
        }
        TestData::only_these_on_base(&td[..9], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(
            &td[..9],
            here!("check all return on same base"),
            &meta_buf,
        );
        td[9].not_on_full_key(here!("removed in scratch"), &meta_buf);
        DatabaseResult::Ok(())
    })
    .unwrap();
}

#[tokio::test(threaded_scheduler)]
async fn links_on_same_tag() {
    observability::test_run().ok();
    let arc = test_cell_env();
    let env = arc.guard().await;

    let mut td = fixtures(10).await;
    let base_hash = td[0].base_hash.clone();
    let zome_id = td[0].zome_id;
    let tag = td[0].tag.clone();

    for d in td.iter_mut() {
        d.base_hash = base_hash.clone();
        d.zome_id = zome_id;
        d.tag = tag.clone();
        d.link_add.base_address = base_hash.clone();
        d.link_add.zome_id = zome_id;
        d.link_add.tag = tag.clone();

        // Create the new hash
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::with_data(Header::LinkAdd(d.link_add.clone()))
                .await
                .unwrap()
                .into();
        d.expected_link.link_add_hash = link_add_hash.clone();
        d.expected_link.zome_id = zome_id;
        d.expected_link.tag = tag.clone();
        d.link_remove.link_add_address = link_add_hash;
    }
    {
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        TestData::only_these_on_base(&td[..], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td[..], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_full_key(
            &td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_half_key(
            &td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        // In db
        TestData::only_these_on_base(&td[..], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td[..], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_full_key(
            &td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_half_key(
            &td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        DatabaseResult::Ok(())
    })
    .unwrap();
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let reader = env.reader().unwrap();
        let mut meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        td[5].remove_link(&mut meta_buf).await;
        td[6].remove_link(&mut meta_buf).await;
        let partial_td = &td[..5].iter().chain(&td[7..]).cloned().collect::<Vec<_>>();
        TestData::only_these_on_base(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_zome_id(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_full_key(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_half_key(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    env.with_reader(|reader| {
        let meta_buf = ChainMetaBuf::primary(&reader, &env).unwrap();
        let partial_td = &td[..5].iter().chain(&td[7..]).cloned().collect::<Vec<_>>();
        TestData::only_these_on_base(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_zome_id(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_full_key(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        TestData::only_these_on_half_key(
            &partial_td[..],
            here!("check all return on same base"),
            &meta_buf,
        );
        DatabaseResult::Ok(())
    })
    .unwrap();
}
