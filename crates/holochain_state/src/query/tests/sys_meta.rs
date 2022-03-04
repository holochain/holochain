#[cfg(test)]
mod tests {
    use ::fixt::prelude::*;
    use holo_hash::fixt::*;
    use holo_hash::*;
    use holochain_types::fixt::HeaderBuilderCommonFixturator;
    use holochain_types::header::NewEntryHeader;
    use holochain_types::{env::DbWrite, fixt::AppEntryTypeFixturator};
    use holochain_zome_types::header;
    use holochain_zome_types::header::builder;
    use holochain_zome_types::header::Delete;
    use holochain_zome_types::header::EntryType;
    use holochain_zome_types::header::HeaderBuilder;
    use holochain_zome_types::header::HeaderBuilderCommon;
    use holochain_zome_types::HeaderHashed;

    struct TestFixtures {
        header_hashes: Box<dyn Iterator<Item = HeaderHash>>,
        entry_hashes: Box<dyn Iterator<Item = EntryHash>>,
        entry_types: Box<dyn Iterator<Item = EntryType>>,
        commons: Box<dyn Iterator<Item = HeaderBuilderCommon>>,
    }

    impl TestFixtures {
        // TODO: fixt: would be nice if this new fn could take a generic Curve
        // and guarantee that the fixturator is an Iterator
        pub fn new() -> Self {
            Self {
                header_hashes: Box::new(HeaderHashFixturator::new(Unpredictable)),
                entry_hashes: Box::new(EntryHashFixturator::new(Unpredictable).map(Into::into)),
                entry_types: Box::new(
                    AppEntryTypeFixturator::new(Unpredictable).map(EntryType::App),
                ),
                commons: Box::new(HeaderBuilderCommonFixturator::new(Unpredictable)),
            }
        }

        pub fn header_hash(&mut self) -> HeaderHash {
            self.header_hashes.next().unwrap()
        }

        pub fn entry_hash(&mut self) -> EntryHash {
            self.entry_hashes.next().unwrap()
        }

        pub fn entry_type(&mut self) -> EntryType {
            self.entry_types.next().unwrap()
        }

        pub fn common(&mut self) -> HeaderBuilderCommon {
            self.commons.next().unwrap()
        }
    }

    async fn test_update(
        original_header_address: HeaderHash,
        entry_hash: EntryHash,
        original_entry_address: EntryHash,
        fx: &mut TestFixtures,
    ) -> (header::Update, HeaderHashed) {
        let builder = builder::Update {
            original_entry_address,
            original_header_address,
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let update = builder.build(fx.common());
        let header = HeaderHashed::from_content_sync(update.clone().into());
        (update, header)
    }

    async fn test_create(
        entry_hash: EntryHash,
        fx: &mut TestFixtures,
    ) -> (header::Create, HeaderHashed) {
        let builder = builder::Create {
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let create = builder.build(fx.common());
        let header = HeaderHashed::from_content_sync(create.clone().into());
        (create, header)
    }

    async fn test_delete(
        deletes_address: HeaderHash,
        deletes_entry_address: EntryHash,
        fx: &mut TestFixtures,
    ) -> (header::Delete, HeaderHashed) {
        let builder = builder::Delete {
            deletes_address,
            deletes_entry_address,
        };
        let delete = builder.build(fx.common());
        let header = HeaderHashed::from_content_sync(delete.clone().into());
        (delete, header)
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_one_hop() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let (update, expected) = test_update(
        //         fx.header_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     buf.register_update(update.clone())?;
        //     let original = update.original_header_address;
        //     let canonical = buf.get_canonical_header_hash(original.clone())?;

        //     assert_eq!(&canonical, expected.as_hash());
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that a header can be redirected three hops
    async fn test_redirect_header_three_hops() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let (update1, header1) = test_update(
        //         fx.header_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update2, header2) = test_update(
        //         header1.into_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update3, expected) = test_update(
        //         header2.into_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let _ = buf.register_update(update1.clone())?;
        //     let _ = buf.register_update(update2)?;
        //     buf.register_update(update3.clone())?;

        //     let original = update1.original_header_address;
        //     let canonical = buf.get_canonical_header_hash(original.clone())?;

        //     assert_eq!(&canonical, expected.as_hash());
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that an entry can be redirected a single hop
    async fn test_redirect_entry_one_hop() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let original_entry = fx.entry_hash();
        //     let header_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;

        //     let (update, _) = test_update(
        //         header_hash,
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let _ = buf.register_update(update.clone())?;

        //     let canonical = buf.get_canonical_entry_hash(original_entry)?;

        //     let expected = update.entry_hash;
        //     assert_eq!(canonical, expected);
        // }
        // Ok(())
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that an entry can be redirected three hops
    async fn test_redirect_entry_three_hops() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let original_entry = fx.entry_hash();
        //     let header_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update1, _) = test_update(
        //         header_hash,
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update2, _) = test_update(
        //         update1.original_header_address.clone(),
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update3, _) = test_update(
        //         update2.original_header_address.clone(),
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let _ = buf.register_update(update1.clone())?;
        //     let _ = buf.register_update(update2.clone())?;
        //     let _ = buf.register_update(update3.clone())?;

        //     let canonical = buf.get_canonical_entry_hash(original_entry)?;

        //     let expected = update3.entry_hash;
        //     assert_eq!(canonical, expected);
        // }
        // Ok(())
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_and_entry() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let original_entry = fx.entry_hash();
        //     let header_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update_header, expected_header) =
        //         test_update(header_hash, fx.entry_hash(), fx.entry_hash(), &mut fx).await;

        //     let original_entry_1 = fx.entry_hash();
        //     let header_hash = test_create(original_entry_1.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update_entry, _) = test_update(
        //         header_hash,
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;

        //     let _ = buf.register_update(update_header.clone())?;
        //     let _ = buf.register_update(update_entry.clone())?;
        //     let expected_entry_hash = update_entry.entry_hash;

        //     let original_header_hash = update_header.original_header_address;
        //     let canonical_header_hash =
        //         buf.get_canonical_header_hash(original_header_hash.clone())?;
        //     let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_1)?;

        //     assert_eq!(&canonical_header_hash, expected_header.as_hash());
        //     assert_eq!(canonical_entry_hash, expected_entry_hash);
        // }
        // Ok(())
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_headers() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let entry_hash = fx.entry_hash();
        // let mut expected: Vec<TimedHeaderHash> = Vec::new();
        // let mut entry_creates: Vec<Create> = Vec::new();
        // for _ in 0..10 as u32 {
        //     let (e, hash) = test_create(entry_hash.clone(), &mut fx).await;
        //     expected.push(hash.into());
        //     entry_creates.push(e)
        // }

        // expected.sort_by_key(|h| h.header_hash.clone());
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         for create in entry_creates {
        //             meta_buf
        //                 .register_header(NewEntryHeader::Create(create))
        //                 .unwrap();
        //         }
        //         let mut headers = meta_buf
        //             .get_headers(&mut reader, entry_hash.clone())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //         arc.conn()
        //             .unwrap()
        //             .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //             .unwrap();
        //     })
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut headers = meta_buf
        //             .get_headers(&mut reader, entry_hash.clone())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_updates() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let original_entry_hash = fx.entry_hash();
        // let original_header_hash = test_create(original_entry_hash.clone(), &mut fx)
        //     .await
        //     .1
        //     .into_inner()
        //     .1;
        // let mut expected: Vec<TimedHeaderHash> = Vec::new();
        // let mut entry_updates = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_update(
        //         original_header_hash.clone(),
        //         fx.entry_hash(),
        //         original_entry_hash.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     expected.push(hash.into());
        //     entry_updates.push(e)
        // }

        // expected.sort_by_key(|h| h.header_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for update in entry_updates {
        //             meta_buf.register_update(update).unwrap();
        //         }
        //         let mut headers = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut headers = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_updates_header() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let original_entry_hash = fx.entry_hash();
        // let original_header_hash = test_create(original_entry_hash.clone(), &mut fx)
        //     .await
        //     .1
        //     .into_inner()
        //     .1;
        // let mut expected: Vec<TimedHeaderHash> = Vec::new();
        // let mut entry_updates = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_update(
        //         original_header_hash.clone(),
        //         fx.entry_hash(),
        //         original_entry_hash.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     expected.push(hash.into());
        //     entry_updates.push(e)
        // }

        // expected.sort_by_key(|h| h.header_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for update in entry_updates {
        //             meta_buf.register_update(update).unwrap();
        //         }
        //         let mut headers = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut headers = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_deletes() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let header_hash = fx.header_hash();
        // let entry_hash = fx.entry_hash();
        // let mut expected: Vec<TimedHeaderHash> = Vec::new();
        // let mut entry_deletes = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_delete(header_hash.clone(), entry_hash.clone(), &mut fx).await;
        //     expected.push(hash.into());
        //     entry_deletes.push(e)
        // }

        // expected.sort_by_key(|h| h.header_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for delete in entry_deletes {
        //             meta_buf.register_delete(delete).unwrap();
        //         }
        //         let mut headers = meta_buf
        //             .get_deletes_on_header(&mut reader, header_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         let mut headers = meta_buf
        //             .get_deletes_on_header(&mut reader, header_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         headers.sort_by_key(|h| h.header_hash.clone());
        //         assert_eq!(headers, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    async fn update_dbs(
        new_entries: &[NewEntryHeader],
        entry_deletes: &[Delete],
        update_entries: &[NewEntryHeader],
        delete_updates: &[Delete],
        _entry_hash: &EntryHash,
        env: DbWrite,
    ) {
        // let mut meta_buf = MetadataBuf::vault(env.clone().into()).unwrap();
        // for e in new_entries.iter().chain(update_entries.iter()) {
        //     meta_buf.register_header(e.clone()).unwrap();
        // }
        // for delete in entry_deletes.iter().chain(delete_updates.iter()) {
        //     meta_buf.register_delete(delete.clone()).unwrap();
        // }
        // env.conn()
        //     .unwrap()
        //     .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //     .unwrap();
        todo!("Write as fact based sql test")
    }

    async fn create_data(
        entry_creates: &mut Vec<NewEntryHeader>,
        entry_deletes: &mut Vec<Delete>,
        entry_updates: &mut Vec<NewEntryHeader>,
        delete_updates: &mut Vec<Delete>,
        entry_hash: &EntryHash,
        fx: &mut TestFixtures,
    ) {
        for _ in 0..10 {
            let (e, h) = test_create(entry_hash.clone(), fx).await;
            entry_creates.push(NewEntryHeader::Create(e));
            let (e, _) = test_delete(h.clone().into_hash(), entry_hash.clone(), fx).await;
            entry_deletes.push(e);
            let (e, h) = test_update(h.into_hash(), entry_hash.clone(), fx.entry_hash(), fx).await;
            entry_updates.push(NewEntryHeader::Update(e));
            let (e, _) = test_delete(h.into_hash(), entry_hash.clone(), fx).await;
            delete_updates.push(e);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_entry_dht_status() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let entry_hash = fx.entry_hash();
        // let mut entry_creates = Vec::new();
        // let mut entry_deletes = Vec::new();
        // let mut entry_updates = Vec::new();
        // let mut delete_updates = Vec::new();

        // create_data(
        //     &mut entry_creates,
        //     &mut entry_deletes,
        //     &mut entry_updates,
        //     &mut delete_updates,
        //     &entry_hash,
        //     &mut fx,
        // )
        // .await;

        // let meta_buf = || MetadataBuf::vault(arc.clone().into()).unwrap();

        // update_dbs(
        //     &entry_creates[..],
        //     &entry_deletes[..0],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Live);
        // });
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[..],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Dead);
        // });
        // // Same headers don't reanimate entry
        // update_dbs(
        //     &entry_creates[..],
        //     &entry_deletes[..0],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Dead);
        // });

        // // Check create bring entry back to life
        // create_data(
        //     &mut entry_creates,
        //     &mut entry_deletes,
        //     &mut entry_updates,
        //     &mut delete_updates,
        //     &entry_hash,
        //     &mut fx,
        // )
        // .await;

        // // New creates should be alive now
        // update_dbs(
        //     &entry_creates[10..],
        //     &entry_deletes[..0],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Live);
        // });

        // // New deletes should be dead
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[10..],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Dead);
        // });
        // // Check update bring entry back to life
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[..0],
        //     &entry_updates[..10],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Live);
        // });
        // // Check deleting update kills entry
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[..0],
        //     &entry_updates[..0],
        //     &delete_updates[..10],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Dead);
        // });
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_entry_dht_status_one_less() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let entry_hash = fx.entry_hash();
        // let mut entry_creates = Vec::new();
        // let mut entry_deletes = Vec::new();
        // let mut entry_updates = Vec::new();
        // let mut delete_updates = Vec::new();

        // create_data(
        //     &mut entry_creates,
        //     &mut entry_deletes,
        //     &mut entry_updates,
        //     &mut delete_updates,
        //     &entry_hash,
        //     &mut fx,
        // )
        // .await;

        // let meta_buf = || MetadataBuf::vault(arc.clone().into()).unwrap();
        // update_dbs(
        //     &entry_creates[..],
        //     &entry_deletes[..0],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Live);
        // });
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[..9],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Live);
        // });
        // update_dbs(
        //     &entry_creates[..0],
        //     &entry_deletes[9..10],
        //     &entry_updates[..0],
        //     &delete_updates[..0],
        //     &entry_hash,
        //     arc.clone(),
        // )
        // .await;
        // fresh_reader_test!(arc, |mut reader| {
        //     let status = meta_buf()
        //         .get_dht_status(&mut reader, &entry_hash.clone().into())
        //         .unwrap();
        //     assert_eq!(status, EntryDhtStatus::Dead);
        // });
        todo!("Write as fact based sql test")
    }
}
