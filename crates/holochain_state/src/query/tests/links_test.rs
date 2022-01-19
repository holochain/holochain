use super::*;
use crate::here;
use crate::prelude::mutations_helpers::insert_valid_integrated_op;
use crate::prelude::*;
use holochain_types::element::SignedHeaderHashedExt;
use holochain_types::env::DbWrite;
use observability;

#[derive(Clone)]
struct TestData {
    link_add: CreateLink,
    link_remove: DeleteLink,
    base_hash: EntryHash,
    zome_id: ZomeId,
    tag: LinkTag,
    expected_link: Link,
    env: DbWrite<DbKindDht>,
    scratch: Scratch,
    query: GetLinksQuery,
    query_no_tag: GetLinksQuery,
}

fn fixtures(env: DbWrite<DbKindDht>, n: usize) -> Vec<TestData> {
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

        let expected_link = Link {
            create_link_hash: link_add_hash.clone(),
            target: target_address.clone(),
            timestamp: link_add.timestamp.clone().into(),
            tag: tag.clone(),
        };

        let link_remove = KnownDeleteLink {
            link_add_address: link_add_hash,
            base_address: link_add.base_address.clone(),
        };
        let link_remove = DeleteLinkFixturator::new(link_remove).next().unwrap();
        let query = GetLinksQuery::tag(
            link_add.base_address.clone(),
            link_add.zome_id,
            link_add.tag.clone(),
        );
        let query_no_tag = GetLinksQuery::base(link_add.base_address.clone(), link_add.zome_id);

        let td = TestData {
            link_add,
            link_remove,
            base_hash: base_address.clone(),
            zome_id,
            tag,
            expected_link,
            env: env.clone(),
            scratch: Scratch::new(),
            query,
            query_no_tag,
        };
        data.push(td);
    }
    data
}

impl TestData {
    /// Create the same test data with a new timestamp
    fn with_same_keys(mut td: Self) -> Self {
        td.link_add.timestamp = holochain_zome_types::Timestamp::now().into();
        let link_add_hash =
            HeaderHashed::from_content_sync(Header::CreateLink(td.link_add.clone())).into_hash();
        td.link_remove.link_add_address = link_add_hash.clone();
        td.expected_link.timestamp = td.link_add.timestamp.clone().into();
        td.expected_link.create_link_hash = link_add_hash;
        td
    }

    fn empty<'a>(&'a self, test: &'static str) {
        let val = fresh_reader_test(self.env.clone(), |txn| {
            self.query
                .run(DbScratch::new(&[&txn], &self.scratch))
                .unwrap()
                .is_empty()
        });
        assert!(val, "{}", test);
    }

    fn only_on_full_key<'a>(&'a self, test: &'static str) {
        let val = fresh_reader_test(self.env.clone(), |txn| {
            self.query
                .run(DbScratch::new(&[&txn], &self.scratch))
                .unwrap()
        });
        assert_eq!(val, &[self.expected_link.clone()], "{}", test);
    }

    fn not_on_full_key<'a>(&'a self, test: &'static str) {
        let val = fresh_reader_test(self.env.clone(), |txn| {
            self.query
                .run(DbScratch::new(&[&txn], &self.scratch))
                .unwrap()
                .contains(&self.expected_link)
        });
        assert!(
            !val,
            "LinkMetaVal: {:?} should not be present {}",
            self.expected_link, test
        );
    }

    fn only_on_base<'a>(&'a self, test: &'static str) {
        let val = fresh_reader_test(self.env.clone(), |txn| {
            self.query_no_tag
                .run(DbScratch::new(&[&txn], &self.scratch))
                .unwrap()
        });
        assert_eq!(val, &[self.expected_link.clone()], "{}", test);
    }

    fn only_on_half_tag<'a>(&'a self, test: &'static str) {
        let tag_len = self.tag.0.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = LinkTag::new(&self.tag.0[..half_tag]);
        let query = GetLinksQuery::tag(self.base_hash.clone(), self.zome_id, half_tag);
        let val = fresh_reader_test(self.env.clone(), |txn| {
            query.run(DbScratch::new(&[&txn], &self.scratch)).unwrap()
        });
        assert_eq!(val, &[self.expected_link.clone()], "{}", test);
    }

    fn is_on_half_tag<'a>(&'a self, test: &'static str) {
        let tag_len = self.tag.0.len();
        // Make sure there is at least some tag
        let half_tag = if tag_len > 1 { tag_len / 2 } else { tag_len };
        let half_tag = LinkTag::new(&self.tag.0[..half_tag]);
        let query = GetLinksQuery::tag(self.base_hash.clone(), self.zome_id, half_tag);
        let val = fresh_reader_test(self.env.clone(), |txn| {
            query
                .run(DbScratch::new(&[&txn], &self.scratch))
                .unwrap()
                .contains(&self.expected_link)
        });
        assert!(
            val,
            "Results should contain LinkMetaVal: {:?} in test: {}",
            self.expected_link, test
        );
    }

    fn add_link(&self) {
        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAddLink(
            fixt!(Signature),
            self.link_add.clone(),
        ));
        self.env
            .conn()
            .unwrap()
            .with_commit_test(|txn| insert_valid_integrated_op(txn, op).unwrap())
            .unwrap();
    }
    fn add_link_scratch(&mut self) {
        let header = SignedHeaderHashed::from_content_sync(SignedHeader(
            Header::CreateLink(self.link_add.clone()),
            fixt!(Signature),
        ));
        self.scratch
            .add_header(Some(fixt!(Zome)), header, ChainTopOrdering::default());
    }
    fn delete_link(&self) {
        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterRemoveLink(
            fixt!(Signature),
            self.link_remove.clone(),
        ));
        self.env
            .conn()
            .unwrap()
            .with_commit_test(|txn| insert_valid_integrated_op(txn, op).unwrap())
            .unwrap();
    }
    fn delete_link_scratch(&mut self) {
        let header = SignedHeaderHashed::from_content_sync(SignedHeader(
            Header::DeleteLink(self.link_remove.clone()),
            fixt!(Signature),
        ));
        self.scratch
            .add_header(Some(fixt!(Zome)), header, ChainTopOrdering::default());
    }
    fn clear_scratch(&mut self) {
        self.scratch.drain_zomed_headers().for_each(|_| ());
    }

    fn only_these_on_base<'a>(td: &'a [Self], test: &'static str) {
        // Check all base hash are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
        }
        let base_hash = td[0].base_hash.clone();
        let expected = td
            .iter()
            .map(|d| d.expected_link.clone())
            .collect::<Vec<_>>();
        let mut val = Vec::new();
        for d in td {
            let query = GetLinksQuery::base(base_hash.clone(), d.zome_id);
            fresh_reader_test(d.env.clone(), |txn| {
                val.extend(
                    query
                        .run(DbScratch::new(&[&txn], &d.scratch))
                        .unwrap()
                        .into_iter(),
                );
            });
        }
        assert_eq!(val, expected, "{}", test);
    }

    fn only_these_on_full_key<'a>(td: &'a [Self], test: &'static str) {
        // Check all base hash, zome_id, tag are the same
        for d in td {
            assert_eq!(d.base_hash, td[0].base_hash, "{}", test);
            assert_eq!(d.zome_id, td[0].zome_id, "{}", test);
            assert_eq!(d.tag, td[0].tag, "{}", test);
        }
        let base_hash = td[0].base_hash.clone();
        let zome_id = td[0].zome_id;
        let tag = td[0].tag.clone();
        let expected = td
            .iter()
            .map(|d| d.expected_link.clone())
            .collect::<Vec<_>>();
        let query = GetLinksQuery::tag(base_hash, zome_id, tag);
        let mut val = Vec::new();
        for d in td {
            fresh_reader_test(d.env.clone(), |txn| {
                val.extend(
                    query
                        .run(DbScratch::new(&[&txn], &d.scratch))
                        .unwrap()
                        .into_iter(),
                );
            });
        }
        assert_eq!(val, expected, "{}", test);
    }

    fn only_these_on_half_key<'a>(td: &'a [Self], test: &'static str) {
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
        let base_hash = td[0].base_hash.clone();
        let zome_id = td[0].zome_id;
        let expected = td
            .iter()
            .map(|d| d.expected_link.clone())
            .collect::<Vec<_>>();
        let query = GetLinksQuery::tag(base_hash, zome_id, half_tag);
        let mut val = Vec::new();
        for d in td {
            fresh_reader_test(d.env.clone(), |txn| {
                val.extend(
                    query
                        .run(DbScratch::new(&[&txn], &d.scratch))
                        .unwrap()
                        .into_iter(),
                );
            });
        }
        assert_eq!(val, expected, "{}", test);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn can_add_and_delete_link() {
    let test_env = test_dht_env();
    let arc = test_env.env();

    let mut td = fixtures(arc.clone(), 1).into_iter().next().unwrap();

    // Check it's empty
    td.empty(here!("empty at start"));

    // Add a link
    // Add
    td.add_link_scratch();
    // Is in scratch
    td.only_on_full_key(here!("add link in scratch"));

    // Remove from scratch
    td.delete_link_scratch();

    // Is empty
    td.empty(here!("empty after remove"));

    let new_td = TestData::with_same_keys(td.clone());
    td = new_td;

    // Add again
    td.add_link_scratch();

    // Is in scratch again
    td.only_on_full_key(here!("Is still in the scratch"));

    // Remove from scratch
    td.delete_link_scratch();

    // Is empty
    td.empty(here!("empty after remove"));

    // Check it's in db
    td.clear_scratch();
    td.add_link();

    td.only_on_full_key(here!("It's in the db"));

    // Remove the link
    td.delete_link();
    // Is empty

    td.empty(here!("empty after remove in db"));

    // Add a link
    let new_td = TestData::with_same_keys(td.clone());
    td = new_td;
    // Add
    td.add_link_scratch();
    // Is in scratch
    td.only_on_full_key(here!("add link in scratch"));
    // No zome, no tag
    td.only_on_base(here!("scratch"));
    // Half the tag
    td.only_on_half_tag(here!("scratch"));

    td.delete_link_scratch();
    td.empty(here!("empty after remove in db"));

    // Partial matching
    td.clear_scratch();
    td.add_link();

    td.only_on_full_key(here!("db"));
    // No zome, no tag
    td.only_on_base(here!("db"));
    // Half the tag
    td.only_on_half_tag(here!("db"));
}

#[tokio::test(flavor = "multi_thread")]
async fn multiple_links() {
    let test_env = test_dht_env();
    let arc = test_env.env();

    let mut td = fixtures(arc.clone().into(), 10);

    // Add links
    {
        // Add
        for d in &mut td {
            d.add_link_scratch();
        }
        // Is in scratch
        for d in &mut td {
            d.only_on_full_key(here!("add link in scratch"));
        }

        // Remove from scratch
        td[5].delete_link_scratch();

        td[5].not_on_full_key(here!("removed in scratch"));

        for d in td[0..5].iter().chain(&td[6..]) {
            d.only_on_full_key(here!("all except 5 scratch"));
        }
        // Can't add back the same header because removes are tombstones
        // so add one with the same key
        let new_td = TestData::with_same_keys(td[5].clone());
        td[5] = new_td;
        // Add again
        td[5].add_link_scratch();

        // Is in scratch again
        td[5].only_on_full_key(here!("Is back in the scratch"));

        for d in &mut td {
            d.only_on_full_key(here!("add link in scratch"));
        }
        for d in &mut td {
            d.clear_scratch();
        }
    }

    {
        for d in &mut td {
            d.add_link();
        }
        for d in &mut td {
            d.only_on_full_key(here!("all in db"));
        }
        td[0].delete_link();

        for d in &td[1..] {
            d.only_on_full_key(here!("all except 0 scratch"));
        }

        td[0].not_on_full_key(here!("removed in scratch"));
    }

    for d in &td[1..] {
        d.only_on_full_key(here!("all except 0"));
    }
    td[0].not_on_full_key(here!("removed in db"));
}
#[tokio::test(flavor = "multi_thread")]
async fn duplicate_links() {
    observability::test_run().ok();
    let test_env = test_dht_env();
    let arc = test_env.env();

    let mut td = fixtures(arc.clone(), 10);
    // Add to db then the same to scratch and expect on one result
    {
        // Add
        for d in &mut td {
            d.add_link_scratch();
        }
        // Is in scratch
        for d in &mut td {
            d.only_on_full_key(here!("re add"));
            // No zome, no tag
            d.only_on_base(here!("re add"));
            // Half the tag
            d.is_on_half_tag(here!("re add"));
        }
        // Add Again
        for d in &mut td {
            d.add_link_scratch();
        }
        // Is in scratch
        for d in &mut td {
            d.only_on_full_key(here!("re add"));
            // No zome, no tag
            d.only_on_base(here!("re add"));
            // Half the tag
            d.is_on_half_tag(here!("re add"));
        }
    }
    {
        // Add
        for d in &mut td {
            d.add_link();
        }
        // Is in scratch
        for d in &mut td {
            d.only_on_full_key(here!("re add"));
            // No zome, no tag
            d.only_on_base(here!("re add"));
            // Half the tag
            d.is_on_half_tag(here!("re add"));
        }
    }

    for d in &mut td {
        d.clear_scratch();
    }
    // Is in db
    for d in &mut td {
        d.only_on_full_key(here!("re add"));
        // No zome, no tag
        d.only_on_base(here!("re add"));
        // Half the tag
        d.is_on_half_tag(here!("re add"));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn links_on_same_base() {
    observability::test_run().ok();
    let test_env = test_dht_env();
    let arc = test_env.env();

    let mut td = fixtures(arc.clone(), 10);
    let base_hash = td[0].base_hash.clone();
    let base_hash = &base_hash;
    for d in td.iter_mut() {
        d.base_hash = base_hash.clone();
        d.link_add.base_address = base_hash.clone();
        // Create the new hash
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::from_content_sync(Header::CreateLink(d.link_add.clone())).into();
        d.expected_link.create_link_hash = link_add_hash.clone();
        d.link_remove.link_add_address = link_add_hash;
        d.link_remove.base_address = base_hash.clone();
        d.query = GetLinksQuery::tag(base_hash.clone(), d.zome_id, d.tag.clone());
        d.query_no_tag = GetLinksQuery::base(base_hash.clone(), d.zome_id);
    }
    {
        // Add
        for d in &mut td {
            d.add_link_scratch();
        }
        // Is in scratch
        for d in &mut td {
            d.only_on_full_key(here!("same base"));
            // Half the tag
            d.is_on_half_tag(here!("same base"));
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"));
    }
    {
        for d in &mut td {
            d.add_link();
        }
        // In db
        for d in &mut td {
            d.only_on_full_key(here!("same base"));
            // Half the tag
            d.is_on_half_tag(here!("same base"));
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"));
    }
    {
        for d in &mut td {
            d.clear_scratch();
        }
        // In db
        for d in &mut td {
            d.only_on_full_key(here!("same base"));
            // Half the tag
            d.is_on_half_tag(here!("same base"));
        }
        TestData::only_these_on_base(&td, here!("check all return on same base"));
    }
    // Check removes etc.
    {
        for d in &mut td {
            d.add_link_scratch();
        }
        td[0].delete_link_scratch();
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"));
            // Half the tag
            d.is_on_half_tag(here!("same base"));
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"));
        td[0].not_on_full_key(here!("removed in scratch"));
    }
    {
        for d in &mut td {
            d.clear_scratch();
        }
        td[0].delete_link();
        for d in &td[1..] {
            d.only_on_full_key(here!("same base"));
            d.is_on_half_tag(here!("same base"));
        }
        TestData::only_these_on_base(&td[1..], here!("check all return on same base"));
        td[0].not_on_full_key(here!("removed in scratch"));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn links_on_same_tag() {
    observability::test_run().ok();
    let test_env = test_dht_env();
    let arc = test_env.env();

    let mut td = fixtures(arc.clone(), 10);
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
        d.link_remove.base_address = base_hash.clone();

        // Create the new hash
        let (_, link_add_hash): (_, HeaderHash) =
            HeaderHashed::from_content_sync(Header::CreateLink(d.link_add.clone())).into();
        d.expected_link.create_link_hash = link_add_hash.clone();
        d.expected_link.tag = tag.clone();
        d.link_remove.link_add_address = link_add_hash;

        d.query = GetLinksQuery::tag(base_hash.clone(), d.zome_id, tag.clone());
        d.query_no_tag = GetLinksQuery::base(base_hash.clone(), d.zome_id);
    }
    {
        // Add
        for d in &mut td {
            d.add_link_scratch();
        }
        TestData::only_these_on_base(&td[..], here!("check all return on same base"));
        TestData::only_these_on_full_key(&td[..], here!("check all return on same base"));
        TestData::only_these_on_half_key(&td[..], here!("check all return on same base"));
    }
    {
        // In db
        TestData::only_these_on_base(&td[..], here!("check all return on same base"));
        TestData::only_these_on_full_key(&td[..], here!("check all return on same base"));
        TestData::only_these_on_half_key(&td[..], here!("check all return on same base"));
    }
    // Check removes etc.
    {
        td[5].delete_link();
        td[6].delete_link();
        let partial_td = &td[..5].iter().chain(&td[7..]).cloned().collect::<Vec<_>>();
        TestData::only_these_on_base(&partial_td[..], here!("check all return on same base"));
        TestData::only_these_on_full_key(&partial_td[..], here!("check all return on same base"));
        TestData::only_these_on_half_key(&partial_td[..], here!("check all return on same base"));
    }
    {
        let partial_td = &td[..5].iter().chain(&td[7..]).cloned().collect::<Vec<_>>();
        TestData::only_these_on_base(&partial_td[..], here!("check all return on same base"));
        TestData::only_these_on_full_key(&partial_td[..], here!("check all return on same base"));
        TestData::only_these_on_half_key(&partial_td[..], here!("check all return on same base"));
    }
}
