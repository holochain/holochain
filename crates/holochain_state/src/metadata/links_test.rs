use super::*;
use crate::here;
use ::fixt::prelude::*;
use holochain_lmdb::buffer::BufferedStore;
use holochain_lmdb::env::EnvironmentWrite;
use holochain_lmdb::fresh_reader_test;
use holochain_lmdb::test_utils::test_cell_env;

use observability;

#[derive(Clone)]
struct TestData {
    link_add: CreateLink,
    link_remove: DeleteLink,
    base_hash: EntryHash,
    zome_id: ZomeId,
    tag: LinkTag,
    expected_link: LinkMetaVal,
    env: EnvironmentWrite,
}

async fn fixtures(env: EnvironmentWrite, n: usize) -> Vec<TestData> {
    let mut tag_fix = BytesFixturator::new(Predictable);
    let mut zome_id = ZomeIdFixturator::new(Predictable);
    let mut data = Vec::new();
    let mut base_hash_fixt = EntryHashFixturator::new(Predictable);
    let mut target_hash_fixt = EntryHashFixturator::new(Unpredictable);
    for _ in 0..n {
        // Create a known link add
        let base_address = base_hash_fixt.next().unwrap();
        let target_address = target_hash_fixt.next().unwrap();

        let tag = LinkTag::new(tag_fix.next().unwrap());
        let zome_id = zome_id.next().unwrap();

        let link_add = KnownCreateLink {
            base_address: base_address.clone(),
            target_address: target_address.clone(),
            zome_id,
            tag: tag.clone(),
        };

        let link_add = CreateLinkFixturator::new(link_add).next().unwrap();

        // Create the expected link result
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::from_content_sync(Header::CreateLink(link_add.clone())).into();

        let expected_link = LinkMetaVal {
            link_add_hash: link_add_hash.clone(),
            target: target_address.clone(),
            timestamp: link_add.timestamp.clone().into(),
            zome_id,
            tag: tag.clone(),
        };

        let link_remove = KnownDeleteLink {
            link_add_address: link_add_hash,
        };
        let link_remove = DeleteLinkFixturator::new(link_remove).next().unwrap();
        let td = TestData {
            link_add,
            link_remove,
            base_hash: base_address.clone(),
            zome_id,
            tag,
            expected_link,
            env: env.clone(),
        };
        data.push(td);
    }
    data
}

impl TestData {
    /// Create the same test data with a new timestamp
    async fn with_same_keys(mut td: Self) -> Self {
        td.link_add.timestamp = timestamp::now().into();
        let link_add_hash =
            HeaderHashed::from_content_sync(Header::CreateLink(td.link_add.clone())).into_hash();
        td.link_remove.link_add_address = link_add_hash.clone();
        td.expected_link.timestamp = td.link_add.timestamp.clone().into();
        td.expected_link.link_add_hash = link_add_hash;
        td
    }

    async fn empty<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &self.tag);
        let val = fresh_reader_test!(self.env, |r| meta_buf
            .get_live_links(&r, &key)
            .unwrap()
            .collect::<Vec<_>>()
            .unwrap()
            .is_empty());
        assert!(val, test);
    }

    #[allow(dead_code)]
    async fn is_on_full_key<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &self.tag);
        assert!(
            fresh_reader_test!(self.env, |r| meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()
                .contains(&self.expected_link)),
            "{}",
            test
        );
    }

    async fn only_on_full_key<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &self.tag);
        fresh_reader_test!(self.env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        ));
    }

    async fn not_on_full_key<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &self.tag);
        assert!(
            fresh_reader_test!(self.env, |r| !meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()
                .contains(&self.expected_link)),
            "LinkMetaVal: {:?} should not be present {}",
            self.expected_link,
            test
        );
    }

    #[allow(dead_code)]
    async fn is_on_base<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::Base(&self.base_hash);
        assert!(
            fresh_reader_test!(self.env, |r| meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()
                .contains(&self.expected_link)),
            "Results should contain LinkMetaVal: {:?} in test: {}",
            self.expected_link,
            test
        );
    }

    async fn only_on_base<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::Base(&self.base_hash);
        fresh_reader_test!(self.env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        ));
    }

    #[allow(dead_code)]
    async fn is_on_zome_id<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZome(&self.base_hash, self.zome_id);
        assert!(
            fresh_reader_test!(self.env, |r| meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()
                .contains(&self.expected_link)),
            "Results should contain LinkMetaVal: {:?} in test: {}",
            self.expected_link,
            test
        );
    }

    async fn only_on_zome_id<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let key = LinkMetaKey::BaseZome(&self.base_hash, self.zome_id);
        fresh_reader_test!(self.env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        ));
    }

    async fn only_on_half_tag<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let tag_len = self.tag.0.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = LinkTag::new(&self.tag.0[..half_tag]);
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &half_tag);
        fresh_reader_test!(self.env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &[self.expected_link.clone()],
            "{}",
            test
        ));
    }

    async fn is_on_half_tag<'a>(&'a self, test: &'static str, meta_buf: &'a MetadataBuf) {
        let tag_len = self.tag.0.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = LinkTag::new(&self.tag.0[..half_tag]);
        let key = LinkMetaKey::BaseZomeTag(&self.base_hash, self.zome_id, &half_tag);
        assert!(
            fresh_reader_test!(self.env, |r| meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()
                .contains(&self.expected_link)),
            "Results should contain LinkMetaVal: {:?} in test: {}",
            self.expected_link,
            test
        );
    }

    async fn add_link(&self, meta_buf: &mut MetadataBuf) {
        meta_buf.add_link(self.link_add.clone()).unwrap();
    }
    async fn delete_link(&self, meta_buf: &mut MetadataBuf) {
        meta_buf.delete_link(self.link_remove.clone()).unwrap();
    }

    #[instrument(skip(td, meta_buf))]
    fn only_these_on_base<'a>(td: &'a [Self], test: &'static str, meta_buf: &'a MetadataBuf) {
        // Check all base hash are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| BytesKey::from(LinkMetaKey::from((&d.0, &d.1.link_add_hash))));
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        let key = LinkMetaKey::Base(&base_hash);
        fresh_reader_test!(td[0].env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        ));
    }

    fn only_these_on_zome_id<'a>(td: &'a [Self], test: &'static str, meta_buf: &'a MetadataBuf) {
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
        expected.sort_by_key(|d| BytesKey::from(LinkMetaKey::from((&d.0, &d.1.link_add_hash))));
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        let key = LinkMetaKey::BaseZome(&base_hash, zome_id);
        fresh_reader_test!(td[0].env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        ));
    }

    fn only_these_on_full_key<'a>(td: &'a [Self], test: &'static str, meta_buf: &'a MetadataBuf) {
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
        expected.sort_by_key(|d| BytesKey::from(LinkMetaKey::from((&d.0, &d.1.link_add_hash))));
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        let key = LinkMetaKey::BaseZomeTag(&base_hash, zome_id, &tag);
        fresh_reader_test!(td[0].env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        ));
    }

    fn only_these_on_half_key<'a>(td: &'a [Self], test: &'static str, meta_buf: &'a MetadataBuf) {
        let tag_len = td[0].tag.0.len();
        // Make sure there is at least some tag
        let tag_len = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = LinkTag::new(&td[0].tag.0[..tag_len]);
        // Check all base hash, zome_id, half tag are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
            assert_eq!(d.zome_id, td[0].zome_id, "{}", test);
            assert_eq!(&d.tag.0[..tag_len], &half_tag.0[..], "{}", test);
        }
        let base_hash = &td[0].base_hash;
        let zome_id = td[0].zome_id;
        let mut expected = td
            .iter()
            .map(|d| (d.link_add.clone(), d.expected_link.clone()))
            .collect::<Vec<_>>();
        expected.sort_by_key(|d| BytesKey::from(LinkMetaKey::from((&d.0, &d.1.link_add_hash))));
        let expected = expected.into_iter().map(|d| d.1).collect::<Vec<_>>();
        let key = LinkMetaKey::BaseZomeTag(&base_hash, zome_id, &half_tag);
        fresh_reader_test!(td[0].env, |r| assert_eq!(
            &meta_buf
                .get_live_links(&r, &key)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap()[..],
            &expected[..],
            "{}",
            test
        ));
    }
}

#[tokio::test(threaded_scheduler)]
async fn can_add_and_delete_link() {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let mut td = fixtures(arc.clone(), 1).await.into_iter().next().unwrap();

    // Check it's empty
    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    td.empty(here!("empty at start"), &meta_buf).await;

    // Add a link
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        td.add_link(&mut meta_buf).await;
        // Is in scratch
        td.only_on_full_key(here!("add link in scratch"), &meta_buf)
            .await;

        // Remove from scratch
        td.delete_link(&mut meta_buf).await;

        // Is empty
        td.empty(here!("empty after remove"), &meta_buf).await;

        let new_td = TestData::with_same_keys(td.clone()).await;
        td = new_td;

        // Add again
        td.add_link(&mut meta_buf).await;

        // Is in scratch again
        td.only_on_full_key(here!("Is still in the scratch"), &meta_buf)
            .await;

        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    // Check it's in db
    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    td.only_on_full_key(here!("It's in the db"), &meta_buf)
        .await;

    // Remove the link
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        td.delete_link(&mut meta_buf).await;
        // Is empty
        td.empty(here!("empty after remove"), &meta_buf).await;
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    // Check it's empty
    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    // Is empty
    td.empty(here!("empty after remove in db"), &meta_buf).await;

    // Add a link
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        let new_td = TestData::with_same_keys(td.clone()).await;
        td = new_td;
        // Add
        td.add_link(&mut meta_buf).await;
        // Is in scratch
        td.only_on_full_key(here!("add link in scratch"), &meta_buf)
            .await;
        // No zome, no tag
        td.only_on_base(here!("scratch"), &meta_buf).await;
        // No tag
        td.only_on_zome_id(here!("scratch"), &meta_buf).await;
        // Half the tag
        td.only_on_half_tag(here!("scratch"), &meta_buf).await;
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    // Partial matching
    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    td.only_on_full_key(here!("db"), &meta_buf).await;
    // No zome, no tag
    td.only_on_base(here!("db"), &meta_buf).await;
    // No tag
    td.only_on_zome_id(here!("db"), &meta_buf).await;
    // Half the tag
    td.only_on_half_tag(here!("db"), &meta_buf).await;
}

#[tokio::test(threaded_scheduler)]
async fn multiple_links() {
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let mut td = fixtures(arc.clone().into(), 10).await;

    // Add links
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("add link in scratch"), &meta_buf)
                .await;
        }

        // Remove from scratch
        td[5].delete_link(&mut meta_buf).await;

        td[5]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;

        for d in td[0..5].iter().chain(&td[6..]) {
            d.only_on_full_key(here!("all except 5 scratch"), &meta_buf)
                .await;
        }
        // Can't add back the same header because removes are tombstones
        // so add one with the same key
        let new_td = TestData::with_same_keys(td[5].clone()).await;
        td[5] = new_td;
        // Add again
        td[5].add_link(&mut meta_buf).await;

        // Is in scratch again
        td[5]
            .only_on_full_key(here!("Is back in the scratch"), &meta_buf)
            .await;

        for d in td.iter() {
            d.only_on_full_key(here!("add link in scratch"), &meta_buf)
                .await;
        }

        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        for d in td.iter() {
            d.only_on_full_key(here!("all in db"), &meta_buf).await;
        }
        td[0].delete_link(&mut meta_buf).await;

        for d in &td[1..] {
            d.only_on_full_key(here!("all except 0 scratch"), &meta_buf)
                .await;
        }

        td[0]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }

    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    for d in &td[1..] {
        d.only_on_full_key(here!("all except 0"), &meta_buf).await;
    }
    td[0]
        .not_on_full_key(here!("removed in db"), &meta_buf)
        .await;
}
#[tokio::test(threaded_scheduler)]
async fn duplicate_links() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let td = fixtures(arc.clone(), 10).await;
    // Add to db then the same to scratch and expect on one result
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("re add"), &meta_buf).await;
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf).await;
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf).await;
        }
        // Add Again
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("re add"), &meta_buf).await;
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf).await;
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf).await;
        }
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("re add"), &meta_buf).await;
            // No zome, no tag
            d.only_on_base(here!("re add"), &meta_buf).await;
            // No tag
            d.only_on_zome_id(here!("re add"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("re add"), &meta_buf).await;
        }
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
    // Is in db
    for d in td.iter() {
        d.only_on_full_key(here!("re add"), &meta_buf).await;
        // No zome, no tag
        d.only_on_base(here!("re add"), &meta_buf).await;
        // No tag
        d.only_on_zome_id(here!("re add"), &meta_buf).await;
        // Half the tag
        d.is_on_half_tag(here!("re add"), &meta_buf).await;
    }
}

#[tokio::test(threaded_scheduler)]
async fn links_on_same_base() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let mut td = fixtures(arc.clone(), 10).await;
    let base_hash = td[0].base_hash.clone();
    let base_hash = &base_hash;
    for d in td.iter_mut() {
        d.base_hash = base_hash.clone();
        d.link_add.base_address = base_hash.clone();
        // Create the new hash
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::from_content_sync(Header::CreateLink(d.link_add.clone())).into();
        d.expected_link.link_add_hash = link_add_hash.clone();
        d.link_remove.link_add_address = link_add_hash;
    }
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            d.only_on_zome_id(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // In db
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            d.only_on_zome_id(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
    }
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        td[0].delete_link(&mut meta_buf).await;
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            d.only_on_zome_id(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"), &meta_buf);
        td[0]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            d.only_on_zome_id(here!("same base"), &meta_buf).await;
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"), &meta_buf);
        td[0]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;
    }
}

#[tokio::test(threaded_scheduler)]
async fn links_on_same_zome_id() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let mut td = fixtures(arc.clone(), 10).await;
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
            HeaderHashed::from_content_sync(Header::CreateLink(d.link_add.clone())).into();
        d.expected_link.link_add_hash = link_add_hash.clone();
        d.expected_link.zome_id = zome_id;
        d.link_remove.link_add_address = link_add_hash;
    }
    {
        let span = debug_span!("check_zome_id");
        let _g = span.enter();
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // Add
        for d in td.iter() {
            d.add_link(&mut meta_buf).await;
        }
        // Is in scratch
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td, here!("check all return on same base"), &meta_buf);
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        // In db
        for d in td.iter() {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(&td, here!("check all return on same base"), &meta_buf);
    }
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        td[9].delete_link(&mut meta_buf).await;
        for d in &td[..9] {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td[..9], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(
            &td[..9],
            here!("check all return on same base"),
            &meta_buf,
        );
        td[9]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;
        env.with_commit(|writer| meta_buf.flush_to_txn(writer))
            .unwrap();
    }
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        for d in &td[..9] {
            d.only_on_full_key(here!("same base"), &meta_buf).await;
            // Half the tag
            d.is_on_half_tag(here!("same base"), &meta_buf).await;
        }
        TestData::only_these_on_base(&td[..9], here!("check all return on same base"), &meta_buf);
        TestData::only_these_on_zome_id(
            &td[..9],
            here!("check all return on same base"),
            &meta_buf,
        );
        td[9]
            .not_on_full_key(here!("removed in scratch"), &meta_buf)
            .await;
    }
}

#[tokio::test(threaded_scheduler)]
async fn links_on_same_tag() {
    observability::test_run().ok();
    let test_env = test_cell_env();
    let arc = test_env.env();
    let env = arc.guard();

    let mut td = fixtures(arc.clone(), 10).await;
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
            HeaderHashed::from_content_sync(Header::CreateLink(d.link_add.clone())).into();
        d.expected_link.link_add_hash = link_add_hash.clone();
        d.expected_link.zome_id = zome_id;
        d.expected_link.tag = tag.clone();
        d.link_remove.link_add_address = link_add_hash;
    }
    {
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
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
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
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
    }
    // Check removes etc.
    {
        let span = debug_span!("check_remove");
        let _g = span.enter();
        let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        td[5].delete_link(&mut meta_buf).await;
        td[6].delete_link(&mut meta_buf).await;
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
    {
        let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
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
    }
}
