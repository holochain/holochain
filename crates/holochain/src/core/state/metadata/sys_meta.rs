#[allow(missing_docs)]
pub enum MetaGetStatus<T> {
    CanonicalHash(T),
    Deleted,
    NotFound,
}

#[cfg(test)]
mod tests {
    use crate::core::state::metadata::{MetadataBuf, MetadataBufT};
    use fallible_iterator::FallibleIterator;
    use fixt::prelude::*;
    use header::{HeaderBuilderCommon, IntendedFor};
    use holo_hash::*;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{
        composite_hash::{EntryHash, HeaderAddress},
        fixt::{AppEntryTypeFixturator, HeaderBuilderCommonFixturator},
        header::{self, builder, EntryType, HeaderBuilder},
        Header, HeaderHashed,
    };

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
                entry_hashes: Box::new(
                    EntryContentHashFixturator::new(Unpredictable).map(Into::into),
                ),
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
        replaces_address: HeaderHash,
        entry_hash: EntryHash,
        intended_for: IntendedFor,
        fx: &mut TestFixtures,
    ) -> (header::EntryUpdate, HeaderHashed) {
        let builder = builder::EntryUpdate {
            intended_for,
            replaces_address,
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let update = builder.build(fx.common());
        let header = HeaderHashed::with_data(update.clone().into())
            .await
            .unwrap();
        (update, header)
    }

    async fn test_create(
        entry_hash: EntryHash,
        fx: &mut TestFixtures,
    ) -> (header::EntryCreate, HeaderHashed) {
        let builder = builder::EntryCreate {
            entry_hash,
            entry_type: fx.entry_type(),
        };
        let create = builder.build(fx.common());
        let header = HeaderHashed::with_data(create.clone().into())
            .await
            .unwrap();
        (create, header)
    }

    async fn test_delete(
        removes_address: HeaderAddress,
        fx: &mut TestFixtures,
    ) -> (header::EntryDelete, HeaderHashed) {
        let builder = builder::EntryDelete { removes_address };
        let delete = builder.build(fx.common());
        let header = HeaderHashed::with_data(delete.clone().into())
            .await
            .unwrap();
        (delete, header)
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = MetadataBuf::primary(&reader, &env)?;
            let (update, expected) = test_update(
                fx.header_hash().into(),
                fx.entry_hash(),
                IntendedFor::Header,
                &mut fx,
            )
            .await;
            buf.add_update(update.clone(), None).await?;
            let original = update.replaces_address;
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(&canonical, expected.as_ref());
        }
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    /// Test that a header can be redirected three hops
    async fn test_redirect_header_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = MetadataBuf::primary(&reader, &env)?;
            let (update1, header1) = test_update(
                fx.header_hash().into(),
                fx.entry_hash(),
                IntendedFor::Header,
                &mut fx,
            )
            .await;
            let (update2, header2) = test_update(
                header1.into_hash().into(),
                fx.entry_hash(),
                IntendedFor::Header,
                &mut fx,
            )
            .await;
            let (update3, expected) = test_update(
                header2.into_hash().into(),
                fx.entry_hash(),
                IntendedFor::Header,
                &mut fx,
            )
            .await;
            let _ = buf.add_update(update1.clone(), None).await?;
            let _ = buf.add_update(update2, None).await?;
            buf.add_update(update3.clone(), None).await?;

            let original = update1.replaces_address;
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(&canonical, expected.as_ref());
        }
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    /// Test that an entry can be redirected a single hop
    async fn test_redirect_entry_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = MetadataBuf::primary(&reader, &env)?;
            let original_entry = fx.entry_hash();
            let header_hash = test_create(original_entry.clone(), &mut fx)
                .await
                .1
                .into_inner()
                .1;

            let (update, _) =
                test_update(header_hash, fx.entry_hash(), IntendedFor::Entry, &mut fx).await;
            let _ = buf
                .add_update(update.clone(), Some(original_entry.clone()))
                .await?;

            let canonical = buf.get_canonical_entry_hash(original_entry)?;

            let expected = update.entry_hash;
            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    /// Test that an entry can be redirected three hops
    async fn test_redirect_entry_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = MetadataBuf::primary(&reader, &env)?;
            let original_entry = fx.entry_hash();
            let header_hash = test_create(original_entry.clone(), &mut fx)
                .await
                .1
                .into_inner()
                .1;
            let (update1, _) =
                test_update(header_hash, fx.entry_hash(), IntendedFor::Entry, &mut fx).await;
            let (update2, _) = test_update(
                update1.replaces_address.clone(),
                fx.entry_hash(),
                IntendedFor::Entry,
                &mut fx,
            )
            .await;
            let (update3, _) = test_update(
                update2.replaces_address.clone(),
                fx.entry_hash(),
                IntendedFor::Entry,
                &mut fx,
            )
            .await;
            let _ = buf
                .add_update(update1.clone(), Some(original_entry.clone()))
                .await?;
            let _ = buf
                .add_update(update2.clone(), Some(original_entry.clone()))
                .await?;
            let _ = buf
                .add_update(update3.clone(), Some(original_entry.clone()))
                .await?;

            let canonical = buf.get_canonical_entry_hash(original_entry)?;

            let expected = update3.entry_hash;
            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    #[ignore]
    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_and_entry() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = MetadataBuf::primary(&reader, &env)?;
            let original_entry = fx.entry_hash();
            let header_hash = test_create(original_entry.clone(), &mut fx)
                .await
                .1
                .into_inner()
                .1;
            let (update_header, expected_header_hash) =
                test_update(header_hash, fx.entry_hash(), IntendedFor::Header, &mut fx).await;

            let original_entry_1 = fx.entry_hash();
            let header_hash = test_create(original_entry_1.clone(), &mut fx)
                .await
                .1
                .into_inner()
                .1;
            let (update_entry, _) =
                test_update(header_hash, fx.entry_hash(), IntendedFor::Entry, &mut fx).await;

            let _ = buf.add_update(update_header.clone(), None).await?;
            let _ = buf
                .add_update(update_entry.clone(), Some(original_entry.clone()))
                .await?;
            let expected_entry_hash = update_entry.entry_hash;

            let original_header_hash = update_header.replaces_address;
            let canonical_header_hash =
                buf.get_canonical_header_hash(original_header_hash.clone())?;
            let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_1)?;

            assert_eq!(&canonical_header_hash, expected_header_hash.as_ref());
            assert_eq!(canonical_entry_hash, expected_entry_hash);
        }
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn add_entry_get_creates() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        let entry_hash = fx.entry_hash();
        let mut expected = Vec::new();
        let mut entry_creates = Vec::new();
        for _ in 0..10 {
            let (e, hash) = test_create(entry_hash.clone(), &mut fx).await;
            let (_, hash) = <(Header, HeaderHash)>::from(hash);
            expected.push(hash);
            entry_creates.push(e)
        }

        expected.sort_by_key(|h| h.clone());
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            for create in entry_creates {
                meta_buf.add_create(create).await.unwrap();
            }
            let mut headers = meta_buf
                .get_creates(entry_hash.clone())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        {
            let reader = env.reader().unwrap();
            let meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            let mut headers = meta_buf
                .get_creates(entry_hash.clone())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn add_entry_get_updates() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        let original_entry_hash = fx.entry_hash();
        let original_header_hash = test_create(original_entry_hash.clone(), &mut fx)
            .await
            .1
            .into_inner()
            .1;
        let mut expected = Vec::new();
        let mut entry_updates = Vec::new();
        for _ in 0..10 {
            let (e, hash) = test_update(
                original_header_hash.clone(),
                fx.entry_hash(),
                IntendedFor::Entry,
                &mut fx,
            )
            .await;
            let (_, hash) = <(Header, HeaderHash)>::from(hash);
            expected.push(hash);
            entry_updates.push(e)
        }

        expected.sort_by_key(|h| h.clone());
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            for update in entry_updates {
                meta_buf
                    .add_update(update, Some(original_entry_hash.clone()))
                    .await
                    .unwrap();
            }
            let mut headers = meta_buf
                .get_updates(original_entry_hash.clone().into())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        {
            let reader = env.reader().unwrap();
            let meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            let mut headers = meta_buf
                .get_updates(original_entry_hash.into())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn add_entry_get_updates_header() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        let original_entry_hash = fx.entry_hash();
        let original_header_hash = test_create(original_entry_hash.clone(), &mut fx)
            .await
            .1
            .into_inner()
            .1;
        let mut expected = Vec::new();
        let mut entry_updates = Vec::new();
        for _ in 0..10 {
            let (e, hash) = test_update(
                original_header_hash.clone(),
                fx.entry_hash(),
                IntendedFor::Header,
                &mut fx,
            )
            .await;
            let (_, hash) = <(Header, HeaderHash)>::from(hash);
            expected.push(hash);
            entry_updates.push(e)
        }

        expected.sort_by_key(|h| h.clone());
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            for update in entry_updates {
                meta_buf.add_update(update, None).await.unwrap();
            }
            let mut headers = meta_buf
                .get_updates(original_header_hash.clone().into())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        {
            let reader = env.reader().unwrap();
            let meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            let mut headers = meta_buf
                .get_updates(original_header_hash.into())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn add_entry_get_deletes() {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        let header_hash = fx.header_hash();
        let mut expected = Vec::new();
        let mut entry_deletes = Vec::new();
        for _ in 0..10 {
            let (e, hash) = test_delete(header_hash.clone(), &mut fx).await;
            let (_, hash) = <(Header, HeaderHash)>::from(hash);
            expected.push(hash);
            entry_deletes.push(e)
        }

        expected.sort_by_key(|h| h.clone());
        {
            let reader = env.reader().unwrap();
            let mut meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            for delete in entry_deletes {
                meta_buf.add_delete(delete).await.unwrap();
            }
            let mut headers = meta_buf
                .get_deletes(header_hash.clone())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
            env.with_commit(|writer| meta_buf.flush_to_txn(writer))
                .unwrap();
        }
        {
            let reader = env.reader().unwrap();
            let meta_buf = MetadataBuf::primary(&reader, &env).unwrap();
            let mut headers = meta_buf
                .get_deletes(header_hash.clone())
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            headers.sort_by_key(|h| h.clone());
            assert_eq!(headers, expected);
        }
    }
}
