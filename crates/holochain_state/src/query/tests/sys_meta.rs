#[cfg(test)]
mod tests {
    use ::fixt::prelude::*;
    use holo_hash::fixt::*;
    use holo_hash::*;
    use holochain_types::action::NewEntryAction;
    use holochain_types::fixt::ActionBuilderCommonFixturator;
    use holochain_types::{env::DbWrite, fixt::AppEntryTypeFixturator};
    use holochain_zome_types::action;
    use holochain_zome_types::action::builder;
    use holochain_zome_types::action::ActionBuilder;
    use holochain_zome_types::action::ActionBuilderCommon;
    use holochain_zome_types::action::Delete;
    use holochain_zome_types::action::EntryType;
    use holochain_zome_types::ActionHashed;

    struct TestFixtures {
        action_hashes: Box<dyn Iterator<Item = ActionHash>>,
        entry_hashes: Box<dyn Iterator<Item = EntryHash>>,
        entry_types: Box<dyn Iterator<Item = EntryType>>,
        commons: Box<dyn Iterator<Item = ActionBuilderCommon>>,
    }

    impl TestFixtures {
        // TODO: fixt: would be nice if this new fn could take a generic Curve
        // and guarantee that the fixturator is an Iterator
        pub fn new() -> Self {
            Self {
                action_hashes: Box::new(ActionHashFixturator::new(Unpredictable)),
                entry_hashes: Box::new(EntryHashFixturator::new(Unpredictable).map(Into::into)),
                entry_types: Box::new(
                    AppEntryTypeFixturator::new(Unpredictable).map(EntryType::App),
                ),
                commons: Box::new(ActionBuilderCommonFixturator::new(Unpredictable)),
            }
        }

        pub fn action_hash(&mut self) -> ActionHash {
            self.action_hashes.next().unwrap()
        }

        pub fn entry_hash(&mut self) -> EntryHash {
            self.entry_hashes.next().unwrap()
        }

        pub fn entry_type(&mut self) -> EntryType {
            self.entry_types.next().unwrap()
        }

        pub fn common(&mut self) -> ActionBuilderCommon {
            self.commons.next().unwrap()
        }
    }

    async fn test_update(
        original_action_address: ActionHash,
        entry_hash: EntryHash,
        original_entry_address: EntryHash,
        fx: &mut TestFixtures,
    ) -> (action::Update, ActionHashed) {
        let builder = builder::Update {
            original_entry_address,
            original_action_address,
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let update = builder.build(fx.common());
        let action = ActionHashed::from_content_sync(update.clone().into());
        (update, action)
    }

    async fn test_create(
        entry_hash: EntryHash,
        fx: &mut TestFixtures,
    ) -> (action::Create, ActionHashed) {
        let builder = builder::Create {
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let create = builder.build(fx.common());
        let action = ActionHashed::from_content_sync(create.clone().into());
        (create, action)
    }

    async fn test_delete(
        deletes_address: ActionHash,
        deletes_entry_address: EntryHash,
        fx: &mut TestFixtures,
    ) -> (action::Delete, ActionHashed) {
        let builder = builder::Delete {
            deletes_address,
            deletes_entry_address,
        };
        let delete = builder.build(fx.common());
        let action = ActionHashed::from_content_sync(delete.clone().into());
        (delete, action)
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that an action can be redirected a single hop
    async fn test_redirect_action_one_hop() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let (update, expected) = test_update(
        //         fx.action_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     buf.register_update(update.clone())?;
        //     let original = update.original_action_address;
        //     let canonical = buf.get_canonical_action_hash(original.clone())?;

        //     assert_eq!(&canonical, expected.as_hash());
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "can't be tested until redirects are implemented"]
    /// Test that an action can be redirected three hops
    async fn test_redirect_action_three_hops() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let (update1, action1) = test_update(
        //         fx.action_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update2, action2) = test_update(
        //         action1.into_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update3, expected) = test_update(
        //         action2.into_hash().into(),
        //         fx.entry_hash(),
        //         fx.entry_hash(),
        //         &mut fx,
        //     )
        //     .await;
        //     let _ = buf.register_update(update1.clone())?;
        //     let _ = buf.register_update(update2)?;
        //     buf.register_update(update3.clone())?;

        //     let original = update1.original_action_address;
        //     let canonical = buf.get_canonical_action_hash(original.clone())?;

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
        //     let action_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;

        //     let (update, _) = test_update(
        //         action_hash,
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
        //     let action_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update1, _) = test_update(
        //         action_hash,
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update2, _) = test_update(
        //         update1.original_action_address.clone(),
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     let (update3, _) = test_update(
        //         update2.original_action_address.clone(),
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
    /// Test that an action can be redirected a single hop
    async fn test_redirect_action_and_entry() -> anyhow::Result<()> {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // {
        //     let mut buf = MetadataBuf::vault(arc.clone().into())?;
        //     let original_entry = fx.entry_hash();
        //     let action_hash = test_create(original_entry.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update_action, expected_action) =
        //         test_update(action_hash, fx.entry_hash(), fx.entry_hash(), &mut fx).await;

        //     let original_entry_1 = fx.entry_hash();
        //     let action_hash = test_create(original_entry_1.clone(), &mut fx)
        //         .await
        //         .1
        //         .into_inner()
        //         .1;
        //     let (update_entry, _) = test_update(
        //         action_hash,
        //         fx.entry_hash(),
        //         original_entry.clone(),
        //         &mut fx,
        //     )
        //     .await;

        //     let _ = buf.register_update(update_action.clone())?;
        //     let _ = buf.register_update(update_entry.clone())?;
        //     let expected_entry_hash = update_entry.entry_hash;

        //     let original_action_hash = update_action.original_action_address;
        //     let canonical_action_hash =
        //         buf.get_canonical_action_hash(original_action_hash.clone())?;
        //     let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_1)?;

        //     assert_eq!(&canonical_action_hash, expected_action.as_hash());
        //     assert_eq!(canonical_entry_hash, expected_entry_hash);
        // }
        // Ok(())
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_actions() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let entry_hash = fx.entry_hash();
        // let mut expected: Vec<TimedActionHash> = Vec::new();
        // let mut entry_creates: Vec<Create> = Vec::new();
        // for _ in 0..10 as u32 {
        //     let (e, hash) = test_create(entry_hash.clone(), &mut fx).await;
        //     expected.push(hash.into());
        //     entry_creates.push(e)
        // }

        // expected.sort_by_key(|h| h.action_hash.clone());
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         for create in entry_creates {
        //             meta_buf
        //                 .register_action(NewEntryAction::Create(create))
        //                 .unwrap();
        //         }
        //         let mut actions = meta_buf
        //             .get_actions(&mut reader, entry_hash.clone())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //         arc.conn()
        //             .unwrap()
        //             .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //             .unwrap();
        //     })
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut actions = meta_buf
        //             .get_actions(&mut reader, entry_hash.clone())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
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
        // let original_action_hash = test_create(original_entry_hash.clone(), &mut fx)
        //     .await
        //     .1
        //     .into_inner()
        //     .1;
        // let mut expected: Vec<TimedActionHash> = Vec::new();
        // let mut entry_updates = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_update(
        //         original_action_hash.clone(),
        //         fx.entry_hash(),
        //         original_entry_hash.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     expected.push(hash.into());
        //     entry_updates.push(e)
        // }

        // expected.sort_by_key(|h| h.action_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for update in entry_updates {
        //             meta_buf.register_update(update).unwrap();
        //         }
        //         let mut actions = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut actions = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_updates_action() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let original_entry_hash = fx.entry_hash();
        // let original_action_hash = test_create(original_entry_hash.clone(), &mut fx)
        //     .await
        //     .1
        //     .into_inner()
        //     .1;
        // let mut expected: Vec<TimedActionHash> = Vec::new();
        // let mut entry_updates = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_update(
        //         original_action_hash.clone(),
        //         fx.entry_hash(),
        //         original_entry_hash.clone(),
        //         &mut fx,
        //     )
        //     .await;
        //     expected.push(hash.into());
        //     entry_updates.push(e)
        // }

        // expected.sort_by_key(|h| h.action_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for update in entry_updates {
        //             meta_buf.register_update(update).unwrap();
        //         }
        //         let mut actions = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     fresh_reader_test!(arc, |mut reader| {
        //         let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //         let mut actions = meta_buf
        //             .get_updates(&mut reader, original_entry_hash.into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn add_entry_get_deletes() {
        // let test_db = test_cell_db();
        // let arc = test_db.env();
        // let mut fx = TestFixtures::new();
        // let action_hash = fx.action_hash();
        // let entry_hash = fx.entry_hash();
        // let mut expected: Vec<TimedActionHash> = Vec::new();
        // let mut entry_deletes = Vec::new();
        // for _ in 0..10 {
        //     let (e, hash) = test_delete(action_hash.clone(), entry_hash.clone(), &mut fx).await;
        //     expected.push(hash.into());
        //     entry_deletes.push(e)
        // }

        // expected.sort_by_key(|h| h.action_hash.clone());
        // {
        //     let mut meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         for delete in entry_deletes {
        //             meta_buf.register_delete(delete).unwrap();
        //         }
        //         let mut actions = meta_buf
        //             .get_deletes_on_action(&mut reader, action_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     });
        //     arc.conn()
        //         .unwrap()
        //         .with_commit(|writer| meta_buf.flush_to_txn(writer))
        //         .unwrap();
        // }
        // {
        //     let meta_buf = MetadataBuf::vault(arc.clone().into()).unwrap();
        //     fresh_reader_test!(arc, |mut reader| {
        //         let mut actions = meta_buf
        //             .get_deletes_on_action(&mut reader, action_hash.clone().into())
        //             .unwrap()
        //             .collect::<Vec<_>>()
        //             .unwrap();
        //         actions.sort_by_key(|h| h.action_hash.clone());
        //         assert_eq!(actions, expected);
        //     })
        // }
        todo!("Write as fact based sql test")
    }

    async fn update_dbs(
        new_entries: &[NewEntryAction],
        entry_deletes: &[Delete],
        update_entries: &[NewEntryAction],
        delete_updates: &[Delete],
        _entry_hash: &EntryHash,
        env: DbWrite,
    ) {
        // let mut meta_buf = MetadataBuf::vault(env.clone().into()).unwrap();
        // for e in new_entries.iter().chain(update_entries.iter()) {
        //     meta_buf.register_action(e.clone()).unwrap();
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
        entry_creates: &mut Vec<NewEntryAction>,
        entry_deletes: &mut Vec<Delete>,
        entry_updates: &mut Vec<NewEntryAction>,
        delete_updates: &mut Vec<Delete>,
        entry_hash: &EntryHash,
        fx: &mut TestFixtures,
    ) {
        for _ in 0..10 {
            let (e, h) = test_create(entry_hash.clone(), fx).await;
            entry_creates.push(NewEntryAction::Create(e));
            let (e, _) = test_delete(h.clone().into_hash(), entry_hash.clone(), fx).await;
            entry_deletes.push(e);
            let (e, h) = test_update(h.into_hash(), entry_hash.clone(), fx.entry_hash(), fx).await;
            entry_updates.push(NewEntryAction::Update(e));
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
        // // Same actions don't reanimate entry
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
