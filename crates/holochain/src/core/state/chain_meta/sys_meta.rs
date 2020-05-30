pub enum MetaGetStatus<T> {
    CanonicalHash(T),
    Deleted,
    NotFound,
}

#[cfg(test)]
mod tests {
    use crate::core::state::chain_meta::{ChainMetaBuf, ChainMetaBufT};
    use fixt::prelude::*;
    use header::HeaderBuilderCommon;
    use holo_hash::*;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_types::{
        composite_hash::{AnyDhtHash, EntryHash},
        fixt::{AppEntryTypeFixturator, HeaderBuilderCommonFixturator},
        header::{self, builder, EntryType, HeaderBuilder, HeaderInner},
        HeaderHashed,
    };
    use unwrap_to::unwrap_to;

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
                header_hashes: Box::new(HeaderHashFixturator::new(Predictable)),
                entry_hashes: Box::new(
                    EntryContentHashFixturator::new(Predictable).map(Into::into),
                ),
                entry_types: Box::new(AppEntryTypeFixturator::new(Predictable).map(EntryType::App)),
                commons: Box::new(HeaderBuilderCommonFixturator::new(Predictable)),
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
        replaces_address: AnyDhtHash,
        entry_hash: EntryHash,
        entry_type: EntryType,
        fx: &mut TestFixtures,
    ) -> (header::EntryUpdate, HeaderHashed) {
        let mut builder = builder::EntryUpdate {
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

    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let (update, _) = test_update(
                fx.header_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let expected = buf.add_update(update.clone())?;
            let original = unwrap_to!(update.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    /// Test that a header can be redirected three hops
    async fn test_redirect_header_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let (update1, header1) = test_update(
                fx.header_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let (update2, header2) = test_update(
                header1.into_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let (update3, _) = test_update(
                header2.into_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let _ = buf.add_update(update1.clone())?;
            let _ = buf.add_update(update2)?;
            let expected = buf.add_update(update3.clone())?;

            let original = unwrap_to!(update1.replaces_address => AnyDhtHash::Header);
            let canonical = buf.get_canonical_header_hash(original.clone())?;

            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    /// Test that an entry can be redirected a single hop
    async fn test_redirect_entry_one_hop() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let (update, _) = test_update(
                fx.entry_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let _ = buf.add_update(update.clone())?;

            let original: EntryHash =
                unwrap_to!(update.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical = buf.get_canonical_entry_hash(original)?;

            let expected = update.entry_hash;
            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    /// Test that an entry can be redirected three hops
    async fn test_redirect_entry_three_hops() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let (update1, _) = test_update(
                fx.entry_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let (update2, _) = test_update(
                update1.replaces_address.clone(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let (update3, _) = test_update(
                update2.replaces_address.clone(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let _ = buf.add_update(update1.clone())?;
            let _ = buf.add_update(update2.clone())?;
            let _ = buf.add_update(update3.clone())?;

            let original: EntryHash =
                unwrap_to!(update1.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical = buf.get_canonical_entry_hash(original)?;

            let expected = update3.entry_hash;
            assert_eq!(canonical, expected);
        }
        Ok(())
    }

    /// Test that a header can be redirected a single hop
    async fn test_redirect_header_and_entry() -> anyhow::Result<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let mut fx = TestFixtures::new();
        {
            let reader = env.reader()?;
            let mut buf = ChainMetaBuf::primary(&reader, &env)?;
            let (update_header, _) = test_update(
                fx.header_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;
            let (update_entry, _) = test_update(
                fx.entry_hash().into(),
                fx.entry_hash(),
                fx.entry_type(),
                &mut fx,
            )
            .await;

            let expected_header_hash = buf.add_update(update_header.clone())?;
            let _ = buf.add_update(update_entry.clone())?;
            let expected_entry_hash = update_entry.entry_hash;

            let original_header_hash =
                unwrap_to!(update_header.replaces_address => AnyDhtHash::Header);
            let original_entry_hash =
                unwrap_to!(update_entry.replaces_address => AnyDhtHash::EntryContent)
                    .clone()
                    .into();
            let canonical_header_hash =
                buf.get_canonical_header_hash(original_header_hash.clone())?;
            let canonical_entry_hash = buf.get_canonical_entry_hash(original_entry_hash)?;

            assert_eq!(canonical_header_hash, expected_header_hash);
            assert_eq!(canonical_entry_hash, expected_entry_hash);
        }
        Ok(())
    }
}
